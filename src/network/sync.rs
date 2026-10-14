//! Nakamoto sync: longest-valid-chain + mempool gossip over HTTP.
//!
//! Peer discovery comes from [`crate::network::membership::Membership`]
//! (UDP/SWIM). This module only moves ledger data over the existing Actix
//! HTTP API using a tiny `TcpStream` client (no extra deps).
//!
//! Steady state: `POST /chain/block` pushes a newly mined block,
//! `POST /chain/pending` pushes new mempool entries. Catch-up: background
//! task polls `GET /chain/height` and pulls `GET /chain` from the tallest
//! same-genesis peer, then applies `replace_chain_if_longer_valid`.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{debug, info, warn};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::api_server::models::{BlockDto, ChainHeightResponse, ChainResponse, PendingResponse};
use crate::blockchain::blockchain::{Block, BlockChain};

const REQ_TIMEOUT: Duration = Duration::from_secs(3);

/// Convert UDP membership addr to HTTP peer addr.
pub fn http_addr_from_udp(udp: SocketAddr, udp_offset: u16) -> Option<SocketAddr> {
    let port = udp.port().checked_sub(udp_offset)?;
    Some(SocketAddr::new(udp.ip(), port))
}

pub fn block_to_dto(block: &Block) -> BlockDto {
    BlockDto {
        nonce: block.nonce(),
        previous_hash: hex::encode(block.previous_hash()),
        timestamp: block.timestamp(),
        transactions: block
            .raw_transactions()
            .iter()
            .map(hex::encode)
            .collect(),
        hash: hex::encode(block.hash()),
    }
}

pub fn dto_to_block(dto: &BlockDto) -> Result<Block, String> {
    let previous_hash =
        hex::decode(dto.previous_hash.trim()).map_err(|_| "bad previous_hash hex".to_string())?;
    let mut txs = Vec::with_capacity(dto.transactions.len());
    for t in dto.transactions.iter() {
        txs.push(hex::decode(t.trim()).map_err(|_| "bad tx hex".to_string())?);
    }
    let block = Block::from_parts(dto.nonce, previous_hash, dto.timestamp, txs);
    let actual = hex::encode(block.hash());
    if actual != dto.hash.trim().to_lowercase() {
        return Err("block hash mismatch".to_string());
    }
    Ok(block)
}

// -- minimal HTTP client --------------------------------------------------

async fn raw_request(host: &str, port: u16, req: &str) -> Result<String, String> {
    let addr = format!("{host}:{port}");
    let mut stream = tokio::time::timeout(REQ_TIMEOUT, tokio::net::TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("timeout connecting {addr}"))?
        .map_err(|e| format!("connect {addr}: {e}"))?;
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("write {addr}: {e}"))?;
    let mut buf = Vec::new();
    tokio::time::timeout(REQ_TIMEOUT, stream.read_to_end(&mut buf))
        .await
        .map_err(|_| format!("timeout reading {addr}"))?
        .map_err(|e| format!("read {addr}: {e}"))?;
    let resp = String::from_utf8_lossy(&buf).into_owned();
    let sep = resp.find("\r\n\r\n").ok_or_else(|| "bad http response".to_string())?;
    let (head, body) = resp.split_at(sep + 4);
    let status_line = head.lines().next().unwrap_or("");
    if !(status_line.contains(" 200 ") || status_line.contains(" 201 ")) {
        return Err(format!("http {status_line} body={}", &body[..body.len().min(200)]));
    }
    Ok(body.to_string())
}

pub async fn http_get(host: &str, port: u16, path: &str) -> Result<String, String> {
    let req = format!(
        "GET {path} HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\nAccept: application/json\r\n\r\n"
    );
    raw_request(host, port, &req).await
}

pub async fn http_post(host: &str, port: u16, path: &str, body: &str) -> Result<String, String> {
    let req = format!(
        "POST {path} HTTP/1.0\r\nHost: {host}:{port}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\nAccept: application/json\r\n\r\n{body}",
        body.len()
    );
    raw_request(host, port, &req).await
}

// -- fetchers ---------------------------------------------------------------

pub async fn fetch_height(peer: &SocketAddr) -> Result<ChainHeightResponse, String> {
    let body = http_get(&peer.ip().to_string(), peer.port(), "/chain/height").await?;
    serde_json::from_str(&body).map_err(|e| format!("decode height: {e}"))
}

pub async fn fetch_chain(peer: &SocketAddr) -> Result<Vec<Block>, String> {
    let body = http_get(&peer.ip().to_string(), peer.port(), "/chain").await?;
    let chain: ChainResponse =
        serde_json::from_str(&body).map_err(|e| format!("decode chain: {e}"))?;
    chain
        .blocks
        .iter()
        .map(dto_to_block)
        .collect::<Result<Vec<_>, _>>()
}

pub async fn fetch_pending(peer: &SocketAddr) -> Result<Vec<Vec<u8>>, String> {
    let body = http_get(&peer.ip().to_string(), peer.port(), "/chain/pending").await?;
    let p: PendingResponse =
        serde_json::from_str(&body).map_err(|e| format!("decode pending: {e}"))?;
    p.pending
        .iter()
        .map(|h| hex::decode(h.trim()).map_err(|_| "bad pending hex".to_string()))
        .collect()
}

// -- sync steps ---------------------------------------------------------------

type ChainCache = Arc<Mutex<HashMap<String, BlockChain>>>;

fn local_snapshot(cache: &ChainCache) -> Option<(usize, String, String)> {
    let guard = cache.lock().ok()?;
    let bc = guard.get("blockchain")?;
    Some((
        bc.height(),
        hex::encode(bc.tip_hash()),
        hex::encode(Block::genesis_hash()),
    ))
}

/// One catch-up round: pull the tallest same-genesis chain, then mempools.
/// Returns a short summary for logs.
pub async fn sync_once(cache: &ChainCache, peers: Vec<SocketAddr>) -> String {
    if peers.is_empty() {
        return "no peers".to_string();
    }
    let Some((local_h, _tip, genesis)) = local_snapshot(cache) else {
        return "no local chain".to_string();
    };
    // 1. Find tallest same-genesis peer.
    let mut best: Option<(SocketAddr, usize)> = None;
    for p in &peers {
        match fetch_height(p).await {
            Ok(h) => {
                if h.genesis_hash != genesis {
                    warn!("sync: peer {p} genesis mismatch, skipping");
                    continue;
                }
                debug!("sync: peer {p} height={}", h.height);
                if h.height > local_h && best.map(|(_, bh)| h.height > bh).unwrap_or(true) {
                    best = Some((*p, h.height));
                }
            }
            Err(e) => debug!("sync: height from {p} failed: {e}"),
        }
    }
    let mut msg = format!("height {local_h}, peers {}", peers.len());
    // 2. Pull full chain from best peer (chains are tiny; header-first is
    //    future work — see GET /chain/headers).
    if let Some((peer, rh)) = best {
        match fetch_chain(&peer).await {
            Ok(blocks) => {
                let n = blocks.len();
                let replaced = cache
                    .lock()
                    .ok()
                    .and_then(|mut g| {
                        g.get_mut("blockchain")
                            .map(|bc| bc.replace_chain_if_longer_valid(blocks))
                    });
                match replaced {
                    Some(Ok(true)) => {
                        info!("sync: adopted chain {local_h}->{n} from {peer} (remote {rh})");
                        msg = format!("adopted {n} from {peer}");
                    }
                    Some(Ok(false)) => msg = format!("peer {peer} not longer"),
                    Some(Err(e)) => {
                        warn!("sync: reject chain from {peer}: {e}");
                        msg = format!("reject from {peer}: {e}");
                    }
                    None => msg = "lock failed".to_string(),
                }
            }
            Err(e) => {
                warn!("sync: fetch chain from {peer} failed: {e}");
                msg = format!("fetch {peer} failed: {e}");
            }
        }
    }
    // 3. Mempool gossip: union of all peers' pools.
    let mut total_new = 0;
    for p in &peers {
        match fetch_pending(p).await {
            Ok(raws) => {
                if let Ok(mut g) = cache.lock() {
                    if let Some(bc) = g.get_mut("blockchain") {
                        total_new += bc.merge_pending_raw(raws);
                    }
                }
            }
            Err(e) => debug!("sync: pending from {p} failed: {e}"),
        }
    }
    if total_new > 0 {
        info!("sync: merged {total_new} mempool txs");
        msg = format!("{msg} +{total_new} txs");
    }
    msg
}

pub async fn broadcast_block(peers: Vec<SocketAddr>, dto: BlockDto) {
    let Ok(body) = serde_json::to_string(&dto) else {
        return;
    };
    for p in peers {
        let b = body.clone();
        tokio::spawn(async move {
            if let Err(e) = http_post(&p.ip().to_string(), p.port(), "/chain/block", &b).await {
                debug!("broadcast block to {p} failed: {e}");
            }
        });
    }
}

pub async fn broadcast_pending(peers: Vec<SocketAddr>, raws: Vec<Vec<u8>>) {
    let pending: Vec<String> = raws.iter().map(hex::encode).collect();
    let Ok(body) = serde_json::to_string(&PendingResponse { pending }) else {
        return;
    };
    for p in peers {
        let b = body.clone();
        tokio::spawn(async move {
            if let Err(e) = http_post(&p.ip().to_string(), p.port(), "/chain/pending", &b).await {
                debug!("broadcast pending to {p} failed: {e}");
            }
        });
    }
}

// -- tests -----------------------------------------------------------------
// T1.3: sync wire-format (block DTO) round-trip + rejection.
#[cfg(test)]
mod tests {
    use super::*;

    fn funded_chain() -> BlockChain {
        let mut bc = BlockChain::new("miner".to_string());
        bc.faucet("alice".to_string(), 100).unwrap();
        bc.mining();
        bc
    }

    #[test]
    fn dto_round_trip_preserves_every_block() {
        let bc = funded_chain();
        for block in bc.get_chain() {
            let dto = block_to_dto(block);
            let back = dto_to_block(&dto).expect("round trip must succeed");
            assert_eq!(back, *block);
            assert_eq!(back.raw_transactions(), block.raw_transactions());
        }
    }

    #[test]
    fn dto_rejects_tampered_fields() {
        let bc = funded_chain();
        let good = block_to_dto(bc.last_block());

        let mut d = good.clone();
        d.nonce += 1;
        assert!(dto_to_block(&d).is_err(), "tampered nonce must fail hash check");

        let mut d = good.clone();
        d.timestamp += 1;
        assert!(
            dto_to_block(&d).is_err(),
            "tampered timestamp must fail hash check"
        );

        let mut d = good.clone();
        d.hash = "00".repeat(32);
        assert!(
            dto_to_block(&d).is_err(),
            "forged hash field must fail hash check"
        );

        let mut d = good.clone();
        d.transactions.push(hex::encode(b"rogue-bytes"));
        assert!(
            dto_to_block(&d).is_err(),
            "injected transaction must fail hash check"
        );
    }

    #[test]
    fn dto_rejects_malformed_hex() {
        let bc = funded_chain();
        let mut d = block_to_dto(bc.last_block());
        d.previous_hash = "zz".to_string();
        assert!(dto_to_block(&d).is_err());

        let mut d = block_to_dto(bc.last_block());
        d.transactions = vec!["zz".to_string()];
        assert!(dto_to_block(&d).is_err());
    }

    #[test]
    fn http_addr_conversion_and_underflow() {
        let udp: SocketAddr = "127.0.0.1:15000".parse().unwrap();
        assert_eq!(
            http_addr_from_udp(udp, 10_000).unwrap().to_string(),
            "127.0.0.1:5000"
        );
        let low: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        assert!(
            http_addr_from_udp(low, 10_000).is_none(),
            "port below offset must not wrap"
        );
    }

    #[test]
    fn chain_response_json_round_trip() {
        let bc = funded_chain();
        let resp = ChainResponse {
            height: bc.height(),
            tip_hash: hex::encode(bc.tip_hash()),
            genesis_hash: hex::encode(Block::genesis_hash()),
            blocks: bc.get_chain().iter().map(block_to_dto).collect(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: ChainResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(back.blocks.len(), bc.height());
        for dto in &back.blocks {
            dto_to_block(dto).expect("wire DTO must decode");
        }
    }
}
