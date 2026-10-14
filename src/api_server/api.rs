//! HTTP API server: routing + handlers only.
//!
//! Layout:
//! - `models.rs` — request/response DTOs (`TransactionRequest`, views, …)
//! - `views.rs` + `templates/` + `static/` — wallet dashboard HTML/CSS/JS
//! - this file — `ApiServer` state, endpoint handlers, route table.

use std::{
    collections::HashMap,
    fmt::Debug,
    net::SocketAddr,
    sync::{Arc, Mutex},
};

use actix_web::{App, HttpResponse, HttpServer, web};
use log::{debug, info};

use crate::blockchain::blockchain::{Block, BlockChain, Serialization};
use crate::blockchain::transaction::Transaction as BlockchainTransaction;
use crate::network::membership::{MemberStatus, Membership};
use crate::network::sync;
use crate::wallet::wallet::{Transaction as WalletTransaction, Wallet};

use super::models::{
    BlockDto, BlockHeaderDto, BlockView, ChainHeightResponse, ChainResponse, FaucetRequest,
    PendingResponse, QueryAmount, TransactionRequest, TransactionView, TransferResponse,
};
use super::views::render_wallet_page;

#[derive(Clone)]
pub struct ApiServer {
    host: String,
    port: u16,
    cache: Arc<Mutex<HashMap<String, BlockChain>>>,
    membership: Option<Arc<Membership>>,
}

impl Debug for ApiServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let height = self
            .cache
            .lock()
            .ok()
            .and_then(|c| c.get("blockchain").map(|b| b.height()));
        f.debug_struct("ApiServer")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("height", &height)
            .field("has_membership", &self.membership.is_some())
            .finish()
    }
}

impl ApiServer {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let api_server = ApiServer {
            host: host.into(),
            port,
            cache,
            membership: None,
        };

        let wallet_miner = Wallet::new();
        {
            // `Mutex::lock` borrows the map; the guard is dropped at the end
            // of this scope, releasing the lock before we return.
            let mut unlock_cache = api_server.cache.lock().unwrap();
            unlock_cache.insert(
                "blockchain".to_string(),
                BlockChain::new(wallet_miner.address_str()),
            );
        }

        api_server
    }

    pub fn set_membership(&mut self, membership: Arc<Membership>) {
        self.membership = Some(membership);
    }

    pub fn cache(&self) -> Arc<Mutex<HashMap<String, BlockChain>>> {
        self.cache.clone()
    }

    pub fn host_str(&self) -> String {
        self.host.clone()
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// HTTP peers derived from SWIM alive UDP addrs (`udp_port - 10_000`).
    /// `udp_offset` is `UDP_PORT_OFFSET` from `network.rs`.
    pub async fn peers_http(&self, udp_offset: u16) -> Vec<SocketAddr> {
        let Some(m) = &self.membership else {
            return vec![];
        };
        m.alive_addrs()
            .await
            .into_iter()
            .filter_map(|u| sync::http_addr_from_udp(u, udp_offset))
            .collect()
    }

    fn build_chain_response(bc: &BlockChain) -> ChainResponse {
        let blocks = bc.get_chain().iter().map(sync::block_to_dto).collect();
        ChainResponse {
            height: bc.height(),
            tip_hash: hex::encode(bc.tip_hash()),
            genesis_hash: hex::encode(Block::genesis_hash()),
            blocks,
        }
    }

    // -- handlers ---------------------------------------------------------

    pub async fn get_amount(
        data: web::Data<Arc<ApiServer>>,
        path: web::Path<String>,
    ) -> HttpResponse {
        let address = path.into_inner();
        let api_server = data.get_ref();
        let unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get("blockchain").unwrap();
        let amount = block_chain.calculate_total_amount(address);

        HttpResponse::Ok().json(QueryAmount { amount })
    }

    pub async fn mining(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        let api_server = data.get_ref();
        let (tip_dto, peers) = {
            let mut unlock_cache = api_server.cache.lock().unwrap();
            let block_chain = unlock_cache.get_mut("blockchain").unwrap();
            if !block_chain.mining() {
                return HttpResponse::InternalServerError().json("mining fail");
            }
            let tip = block_chain.last_block().clone();
            let dto = sync::block_to_dto(&tip);
            let tip_hash = hex::encode(tip.hash());
            info!("mined new tip {tip_hash} height {}", block_chain.height());
            (dto, api_server.membership.clone())
        };
        // Push fast-path (best-effort); pull loop covers failures.
        if let Some(m) = peers {
            tokio::spawn(async move {
                let udp_peers = m.alive_addrs().await;
                let http_peers: Vec<SocketAddr> = udp_peers
                    .into_iter()
                    .filter_map(|u| {
                        u.port()
                            .checked_sub(crate::network::network::UDP_PORT_OFFSET)
                            .map(|p| SocketAddr::new(u.ip(), p))
                    })
                    .collect();
                sync::broadcast_block(http_peers, tip_dto).await;
            });
        }

        HttpResponse::Ok().json("mining ok")
    }

    pub async fn faucet(
        data: web::Data<Arc<ApiServer>>,
        req: web::Json<FaucetRequest>,
    ) -> HttpResponse {
        let recipient = req.recipient_address.trim().to_string();
        if recipient.is_empty() {
            return HttpResponse::BadRequest().json("recipient_address must not be empty");
        }
        let amount: u64 = match req.amount.trim().parse() {
            Ok(v) => v,
            Err(_) => return HttpResponse::BadRequest().json("amount must be a positive integer"),
        };
        if amount == 0 {
            return HttpResponse::BadRequest().json("amount must be greater than 0");
        }

        let api_server = data.get_ref();
        let mut unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get_mut("blockchain").unwrap();
        match block_chain.faucet(recipient.clone(), amount) {
            Ok(()) => {
                info!("faucet: minted {amount} to {recipient}");
                let pending = block_chain.pending_raw();
                let membership = api_server.membership.clone();
                drop(unlock_cache);
                if let Some(m) = membership {
                    tokio::spawn(async move {
                        let udp_peers = m.alive_addrs().await;
                        let http_peers: Vec<SocketAddr> = udp_peers
                            .into_iter()
                            .filter_map(|u| {
                                u.port()
                                    .checked_sub(crate::network::network::UDP_PORT_OFFSET)
                                    .map(|p| SocketAddr::new(u.ip(), p))
                            })
                            .collect();
                        sync::broadcast_pending(http_peers, pending).await;
                    });
                }
                // Re-lock for the count (cheap, avoids holding across await).
                let count = data
                    .get_ref()
                    .cache
                    .lock()
                    .ok()
                    .and_then(|c| c.get("blockchain").map(|b| b.pending_count()))
                    .unwrap_or(0);
                HttpResponse::Ok().json(serde_json::json!({
                    "status": "ok",
                    "recipient": recipient,
                    "amount": amount,
                    "pending_in_pool": count,
                    "note": "mine to confirm",
                }))
            }
            Err(e) => HttpResponse::BadRequest().json(e),
        }
    }

    /// Serves the wallet dashboard. Markup/CSS/JS live in
    /// `templates/wallet.html`, `static/wallet.css`, `static/wallet.js`
    /// and are composed by `views::render_wallet_page`.
    async fn get_wallet() -> HttpResponse {
        HttpResponse::Ok()
            .content_type("text/html")
            .body(render_wallet_page())
    }

    pub async fn get_transaction_handler(
        data: web::Data<Arc<ApiServer>>,
        transaction: web::Json<TransactionRequest>,
    ) -> HttpResponse {
        let tx = transaction.into_inner();

        let sender = tx.blockchain_address.trim().to_string();
        let recipient = tx.recipient_address.trim().to_string();
        if sender.is_empty() || recipient.is_empty() {
            return HttpResponse::BadRequest().json("sender and recipient must not be empty");
        }
        if sender == recipient {
            return HttpResponse::BadRequest().json("sender and recipient must differ");
        }
        let amount: u64 = match tx.amount.trim().parse() {
            Ok(v) => v,
            Err(_) => return HttpResponse::BadRequest().json("amount must be a positive integer"),
        };
        if amount == 0 {
            return HttpResponse::BadRequest().json("amount must be greater than 0");
        }

        // The client signed this transfer locally (browser WebCrypto); the
        // server only verifies. No private key ever reaches us — log routing
        // metadata only, never key material.
        debug!("queue tx {sender} -> {recipient} : {amount}");
        let wallet_tx = WalletTransaction {
            sender: sender.clone(),
            recipeint: recipient.clone(),
            amount,
            public_key: tx.public_key.trim().to_string(),
            signature: tx.signature.trim().to_string(),
        };

        let api_server = data.get_ref();
        let mut unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get_mut("blockchain").unwrap();
        match block_chain.add_transaction(&wallet_tx) {
            Ok(()) => {
                info!("queued transaction {sender} -> {recipient} : {amount}");
                let pending = block_chain.pending_raw();
                let count = block_chain.pending_count();
                let membership = api_server.membership.clone();
                drop(unlock_cache);
                if let Some(m) = membership {
                    tokio::spawn(async move {
                        let udp_peers = m.alive_addrs().await;
                        let http_peers: Vec<SocketAddr> = udp_peers
                            .into_iter()
                            .filter_map(|u| {
                                u.port()
                                    .checked_sub(crate::network::network::UDP_PORT_OFFSET)
                                    .map(|p| SocketAddr::new(u.ip(), p))
                            })
                            .collect();
                        sync::broadcast_pending(http_peers, pending).await;
                    });
                }
                HttpResponse::Created().json(TransferResponse {
                    status: "pending".to_string(),
                    sender,
                    recipient,
                    amount,
                    pending_in_pool: count,
                    note: "call GET /mining to confirm, then check /amount/{address}".to_string(),
                })
            }
            Err(e) if e.contains("insufficient") => {
                HttpResponse::UnprocessableEntity().json(e)
            }
            Err(e) => HttpResponse::BadRequest().json(e),
        }
    }

    pub async fn show_transaction(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        let api_server = data.get_ref();
        let unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get("blockchain").unwrap();
        let confirmed: Vec<TransactionView> = block_chain
            .get_transactions()
            .iter()
            .map(TransactionView::from)
            .collect();
        let count = confirmed.len();
        debug!("show transactions in chain:{:?}", confirmed);
        HttpResponse::Ok().json(serde_json::json!({
            "transaction_count": count,
            "transactions": confirmed,
            "pending_in_pool": block_chain.pending_count(),
        }))
    }

    async fn get_wallet_handler() -> HttpResponse {
        let wallet_user = Wallet::new();
        let wallet_data = wallet_user.get_wallet_data();
        HttpResponse::Ok().json(wallet_data)
    }

    async fn get_index(&self) -> HttpResponse {
        let unlock_cache = self.cache.lock().unwrap();
        let blockchain = unlock_cache.get("blockchain").unwrap();
        let views: Vec<BlockView> = blockchain
            .get_chain()
            .iter()
            .enumerate()
            .map(|(index, b)| {
                let transactions: Vec<TransactionView> = b
                    .raw_transactions()
                    .iter()
                    .map(|raw| {
                        let tx = BlockchainTransaction::deserialization(raw.clone());
                        TransactionView::from(&tx)
                    })
                    .collect();
                BlockView {
                    index,
                    nonce: b.nonce(),
                    previous_hash: hex::encode(b.previous_hash()),
                    hash: hex::encode(b.hash()),
                    timestamp: b.timestamp(),
                    transactions,
                }
            })
            .collect();
        HttpResponse::Ok().json(views)
    }

    pub async fn get_index_handler(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        info!("Receiving request at '/' endpoint");
        debug!("Handler received ApiServer data: {:?}", data);

        data.get_ref().get_index().await
    }

    // -- Nakamoto sync endpoints -------------------------------------------

    pub async fn chain_height(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        let guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get("blockchain").unwrap();
        HttpResponse::Ok().json(ChainHeightResponse {
            height: bc.height(),
            tip_hash: hex::encode(bc.tip_hash()),
            genesis_hash: hex::encode(Block::genesis_hash()),
        })
    }

    pub async fn chain_full(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        let guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get("blockchain").unwrap();
        HttpResponse::Ok().json(Self::build_chain_response(bc))
    }

    pub async fn chain_headers(
        data: web::Data<Arc<ApiServer>>,
        query: web::Query<HashMap<String, String>>,
    ) -> HttpResponse {
        let guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get("blockchain").unwrap();
        let chain = bc.get_chain();
        let limit = query
            .get("limit")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(128)
            .min(1024);
        let start: usize = match query.get("from") {
            None => 0,
            Some(h) => match hex::decode(h.trim()) {
                Ok(hash) => chain
                    .iter()
                    .position(|b| b.hash() == hash)
                    .map(|i| i + 1)
                    .unwrap_or(0),
                Err(_) => return HttpResponse::BadRequest().json("bad from hash hex"),
            },
        };
        let headers: Vec<BlockHeaderDto> = chain
            .iter()
            .enumerate()
            .skip(start)
            .take(limit)
            .map(|(index, b)| BlockHeaderDto {
                index,
                hash: hex::encode(b.hash()),
                previous_hash: hex::encode(b.previous_hash()),
                nonce: b.nonce(),
                timestamp: b.timestamp(),
                tx_count: b.raw_transactions().len(),
            })
            .collect();
        HttpResponse::Ok().json(headers)
    }

    pub async fn chain_block(
        data: web::Data<Arc<ApiServer>>,
        path: web::Path<String>,
    ) -> HttpResponse {
        let Ok(hash) = hex::decode(path.into_inner().trim()) else {
            return HttpResponse::BadRequest().json("bad hash hex");
        };
        let guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get("blockchain").unwrap();
        match bc.block_by_hash(&hash) {
            Some((_, b)) => HttpResponse::Ok().json(sync::block_to_dto(b)),
            None => HttpResponse::NotFound().json("block not found"),
        }
    }

    /// Push fast-path: accept a block that directly extends our tip.
    /// Anything else gets a 409 — the background pull loop will fetch the
    /// full longest chain within seconds.
    pub async fn submit_block(
        data: web::Data<Arc<ApiServer>>,
        body: web::Json<BlockDto>,
    ) -> HttpResponse {
        let block = match sync::dto_to_block(&body.into_inner()) {
            Ok(b) => b,
            Err(e) => return HttpResponse::BadRequest().json(e),
        };
        let mut guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get_mut("blockchain").unwrap();
        if block.hash() == bc.tip_hash() {
            return HttpResponse::Ok().json(serde_json::json!({"status": "duplicate"}));
        }
        match bc.append_block_if_extends(block) {
            Ok(true) => {
                info!("sync: appended pushed tip, height {}", bc.height());
                HttpResponse::Ok().json(serde_json::json!({"status": "appended"}))
            }
            Ok(false) => HttpResponse::Conflict().json("not appended"),
            Err(e) => {
                debug!("sync: reject pushed block: {e}");
                HttpResponse::Conflict().json(format!("need sync: {e}"))
            }
        }
    }

    pub async fn get_pending(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        let guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get("blockchain").unwrap();
        let pending = bc.pending_raw().iter().map(hex::encode).collect();
        HttpResponse::Ok().json(PendingResponse { pending })
    }

    pub async fn submit_pending(
        data: web::Data<Arc<ApiServer>>,
        body: web::Json<PendingResponse>,
    ) -> HttpResponse {
        let mut raws = Vec::with_capacity(body.pending.len());
        for h in body.into_inner().pending {
            match hex::decode(h.trim()) {
                Ok(r) => raws.push(r),
                Err(_) => return HttpResponse::BadRequest().json("bad tx hex"),
            }
        }
        let mut guard = data.get_ref().cache.lock().unwrap();
        let bc = guard.get_mut("blockchain").unwrap();
        let added = bc.merge_pending_raw(raws);
        HttpResponse::Ok().json(serde_json::json!({"added": added}))
    }

    pub async fn peers(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        let api = data.get_ref();
        let Some(m) = &api.membership else {
            return HttpResponse::Ok().json(serde_json::json!({"peers": []}));
        };
        let list = m.member_list().await;
        let alive: Vec<String> = list
            .iter()
            .filter(|u| u.status == MemberStatus::Alive)
            .map(|u| u.addr.to_string())
            .collect();
        HttpResponse::Ok().json(serde_json::json!({
            "self_udp": m.self_addr().to_string(),
            "alive": alive,
        }))
    }

    // -- routing ----------------------------------------------------------

    fn configure_routes(cfg: &mut web::ServiceConfig) {
        cfg.route("/", web::get().to(Self::get_index_handler))
            .route("/wallet", web::get().to(Self::get_wallet))
            .route("/get_wallet", web::get().to(Self::get_wallet_handler))
            .route(
                "/transaction",
                web::post().to(Self::get_transaction_handler),
            )
            .route("/faucet", web::post().to(Self::faucet))
            .route("/show_transactions", web::get().to(Self::show_transaction))
            .route("/mining", web::get().to(Self::mining))
            .route("/mining", web::post().to(Self::mining))
            .route("/amount/{address}", web::get().to(Self::get_amount))
            .route("/chain/height", web::get().to(Self::chain_height))
            .route("/chain", web::get().to(Self::chain_full))
            .route("/chain/headers", web::get().to(Self::chain_headers))
            .route("/chain/block/{hash}", web::get().to(Self::chain_block))
            .route("/chain/block", web::post().to(Self::submit_block))
            .route("/chain/pending", web::get().to(Self::get_pending))
            .route("/chain/pending", web::post().to(Self::submit_pending))
            .route("/peers", web::get().to(Self::peers));
    }

    pub async fn run(&self) {
        // Share state across Actix worker threads via `Arc`.
        let api = Arc::new(self.clone());

        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(api.clone()))
                .wrap(actix_web::middleware::Logger::default())
                .configure(Self::configure_routes)
        });

        println!("Server running on {}:{}", self.host, self.port);
        server
            .bind((self.host.as_str(), self.port))
            .unwrap()
            .run()
            .await
            .expect("Error running the server");
    }
}

// -- tests -----------------------------------------------------------------
// T1.6: API happy paths + bad input, through the real route table.
#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;

    macro_rules! test_app {
        () => {
            test::init_service(
                actix_web::App::new()
                    .app_data(web::Data::new(std::sync::Arc::new(ApiServer::new(
                        "127.0.0.1",
                        0,
                    ))))
                    .configure(ApiServer::configure_routes),
            )
            .await
        };
    }

    async fn json_body(resp: actix_web::dev::ServiceResponse) -> serde_json::Value {
        let body = test::read_body(resp).await;
        serde_json::from_slice(&body).expect("response must be JSON")
    }

    async fn post_json(
        app: &impl actix_web::dev::Service<
            actix_http::Request,
            Response = actix_web::dev::ServiceResponse,
            Error = actix_web::Error,
        >,
        uri: &str,
        payload: &serde_json::Value,
    ) -> actix_web::dev::ServiceResponse {
        let req = test::TestRequest::post()
            .uri(uri)
            .set_json(payload)
            .to_request();
        test::call_service(app, req).await
    }

    // Build a /transaction body the way the browser does: signed locally,
    // no private key on the wire.
    fn signed_tx_payload(w: &Wallet, recipient: &str, amount: u64) -> serde_json::Value {
        let wt = w.sign_transaction(&recipient.to_string(), amount);
        serde_json::json!({
            "public_key": wt.public_key,
            "blockchain_address": wt.sender,
            "recipient_address": wt.recipeint,
            "amount": wt.amount.to_string(),
            "signature": wt.signature,
        })
    }

    #[actix_web::test]
    async fn get_wallet_returns_key_triple() {
        let app = test_app!();
        let req = test::TestRequest::get().uri("/get_wallet").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body = json_body(resp).await;
        for field in ["public_key", "private_key", "blockchain_address"] {
            assert!(
                body[field].as_str().is_some_and(|s| !s.is_empty()),
                "missing {field}"
            );
        }
    }

    #[actix_web::test]
    async fn faucet_rejects_bad_input() {
        let app = test_app!();
        for (recipient, amount) in [
            ("", "10"),    // empty
            ("   ", "10"), // blank
            ("alice", "0"),
            ("alice", "abc"),
            ("alice", "-5"),
            ("alice", ""),
        ] {
            let resp = post_json(
                &app,
                "/faucet",
                &serde_json::json!({"recipient_address": recipient, "amount": amount}),
            )
            .await;
            assert_eq!(
                resp.status().as_u16(),
                400,
                "faucet {recipient:?}/{amount:?} must be 400"
            );
        }
    }

    #[actix_web::test]
    async fn faucet_then_mine_confirms_balance() {
        let app = test_app!();
        let resp = post_json(
            &app,
            "/faucet",
            &serde_json::json!({"recipient_address": "alice", "amount": "50"}),
        )
        .await;
        assert_eq!(resp.status().as_u16(), 200);

        // Unconfirmed before mining.
        let req = test::TestRequest::get().uri("/amount/alice").to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["amount"], 0);

        let req = test::TestRequest::post().uri("/mining").to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        let req = test::TestRequest::get().uri("/amount/alice").to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["amount"], 50);
    }

    #[actix_web::test]
    async fn transaction_rejects_bad_input() {
        let app = test_app!();
        let w = Wallet::new();
        let addr = w.address_str();
        for (sender, recipient, amount) in [
            ("", "bob", "10"),          // empty sender
            (addr.as_str(), "", "10"),  // empty recipient
            (addr.as_str(), addr.as_str(), "10"), // self-transfer
            (addr.as_str(), "bob", "0"),
            (addr.as_str(), "bob", "abc"),
        ] {
            let mut payload = signed_tx_payload(&w, recipient, 10);
            payload["blockchain_address"] = serde_json::json!(sender);
            payload["recipient_address"] = serde_json::json!(recipient);
            payload["amount"] = serde_json::json!(amount);
            let resp = post_json(&app, "/transaction", &payload).await;
            assert_eq!(
                resp.status().as_u16(),
                400,
                "transaction {sender:?}/{recipient:?}/{amount:?} must be 400"
            );
        }
        // Tampered amount breaks the client signature.
        let mut tampered = signed_tx_payload(&w, "bob", 10);
        tampered["amount"] = serde_json::json!("11");
        let resp = post_json(&app, "/transaction", &tampered).await;
        assert_eq!(resp.status().as_u16(), 400);
        // Garbage public key must not panic: it becomes a 400.
        let mut bad_key = signed_tx_payload(&w, "bob", 10);
        bad_key["public_key"] = serde_json::json!("zz");
        let resp = post_json(&app, "/transaction", &bad_key).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn transaction_full_flow_confirms_after_mine() {
        let app = test_app!();
        let sender = Wallet::new();
        let recipient = Wallet::new();
        let s_addr = sender.address_str();
        let r_addr = recipient.address_str();

        post_json(
            &app,
            "/faucet",
            &serde_json::json!({"recipient_address": s_addr, "amount": "50"}),
        )
        .await;
        let req = test::TestRequest::post().uri("/mining").to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        let resp = post_json(&app, "/transaction", &signed_tx_payload(&sender, &r_addr, 20)).await;
        assert_eq!(resp.status().as_u16(), 201);

        // Queued but unconfirmed.
        let req = test::TestRequest::get().uri("/show_transactions").to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["pending_in_pool"], 1);

        let req = test::TestRequest::post().uri("/mining").to_request();
        assert_eq!(test::call_service(&app, req).await.status().as_u16(), 200);

        let req = test::TestRequest::get()
            .uri(&format!("/amount/{s_addr}"))
            .to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["amount"], 30);

        let req = test::TestRequest::get()
            .uri(&format!("/amount/{r_addr}"))
            .to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["amount"], 20);
    }

    #[actix_web::test]
    async fn transaction_rejects_insufficient_funds() {
        let app = test_app!();
        let sender = Wallet::new();
        let s_addr = sender.address_str();
        post_json(
            &app,
            "/faucet",
            &serde_json::json!({"recipient_address": s_addr, "amount": "10"}),
        )
        .await;
        let req = test::TestRequest::post().uri("/mining").to_request();
        test::call_service(&app, req).await;

        let resp = post_json(&app, "/transaction", &signed_tx_payload(&sender, "bob", 999)).await;
        assert_eq!(resp.status().as_u16(), 422);
    }

    #[actix_web::test]
    async fn mining_returns_ok() {
        let app = test_app!();
        let req = test::TestRequest::post().uri("/mining").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(json_body(resp).await, "mining ok");
    }

    #[actix_web::test]
    async fn chain_height_and_full_are_consistent() {
        let app = test_app!();
        let req = test::TestRequest::get().uri("/chain/height").to_request();
        let height = json_body(test::call_service(&app, req).await).await;
        assert_eq!(height["height"], 2); // genesis + first reward
        assert!(!height["tip_hash"].as_str().unwrap().is_empty());
        assert!(!height["genesis_hash"].as_str().unwrap().is_empty());

        let req = test::TestRequest::get().uri("/chain").to_request();
        let full = json_body(test::call_service(&app, req).await).await;
        assert_eq!(full["height"], 2);
        assert_eq!(full["blocks"].as_array().unwrap().len(), 2);
        assert_eq!(full["tip_hash"], height["tip_hash"]);
    }

    #[actix_web::test]
    async fn chain_headers_pagination_and_bad_from() {
        let app = test_app!();
        let req = test::TestRequest::get()
            .uri("/chain/headers?limit=1")
            .to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body.as_array().unwrap().len(), 1);

        let req = test::TestRequest::get().uri("/chain/headers").to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body.as_array().unwrap().len(), 2);

        let req = test::TestRequest::get()
            .uri("/chain/headers?from=zz")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn chain_block_lookup_and_not_found() {
        let app = test_app!();
        let req = test::TestRequest::get().uri("/chain/height").to_request();
        let height = json_body(test::call_service(&app, req).await).await;
        let tip = height["tip_hash"].as_str().unwrap().to_string();

        let req = test::TestRequest::get()
            .uri(&format!("/chain/block/{tip}"))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);

        let req = test::TestRequest::get()
            .uri(&format!("/chain/block/{}", "00".repeat(32)))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 404);

        let req = test::TestRequest::get()
            .uri("/chain/block/zz")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn submit_block_duplicate_reject_and_foreign() {
        let app = test_app!();
        // Current tip DTO round-trips through the wire format.
        let req = test::TestRequest::get().uri("/chain").to_request();
        let full = json_body(test::call_service(&app, req).await).await;
        let blocks = full["blocks"].as_array().unwrap().clone();
        let tip_dto = blocks.last().unwrap().clone();

        // Re-pushing our own tip is a harmless duplicate.
        let resp = post_json(&app, "/chain/block", &tip_dto).await;
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(json_body(resp).await["status"], "duplicate");

        // Tampered DTO fails the hash check.
        let mut tampered = tip_dto.clone();
        tampered["nonce"] = serde_json::json!(tip_dto["nonce"].as_i64().unwrap() + 1);
        let resp = post_json(&app, "/chain/block", &tampered).await;
        assert_eq!(resp.status().as_u16(), 400);

        // Genesis does not extend the tip: 409, pull loop owns catch-up.
        let resp = post_json(&app, "/chain/block", &blocks[0]).await;
        assert_eq!(resp.status().as_u16(), 409);
    }

    #[actix_web::test]
    async fn pending_roundtrip_and_bad_hex() {
        let app = test_app!();
        let req = test::TestRequest::get().uri("/chain/pending").to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["pending"].as_array().unwrap().len(), 0);

        post_json(
            &app,
            "/faucet",
            &serde_json::json!({"recipient_address": "alice", "amount": "5"}),
        )
        .await;
        let req = test::TestRequest::get().uri("/chain/pending").to_request();
        let body = json_body(test::call_service(&app, req).await).await;
        assert_eq!(body["pending"].as_array().unwrap().len(), 1);

        // Re-pushing our own entry merges nothing but stays 200.
        let resp = post_json(&app, "/chain/pending", &body).await;
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(json_body(resp).await["added"], 0);

        let resp = post_json(
            &app,
            "/chain/pending",
            &serde_json::json!({"pending": ["zz"]}),
        )
        .await;
        assert_eq!(resp.status().as_u16(), 400);
    }

    #[actix_web::test]
    async fn peers_without_membership_is_empty() {
        let app = test_app!();
        let req = test::TestRequest::get().uri("/peers").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        assert_eq!(json_body(resp).await["peers"], serde_json::json!([]));
    }

    #[actix_web::test]
    async fn index_and_wallet_page_serve() {
        let app = test_app!();
        let req = test::TestRequest::get().uri("/").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        let body = json_body(resp).await;
        assert_eq!(body.as_array().unwrap().len(), 2);

        let req = test::TestRequest::get().uri("/wallet").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status().as_u16(), 200);
        assert!(
            resp.headers()
                .get("content-type")
                .unwrap()
                .to_str()
                .unwrap()
                .contains("text/html")
        );
    }
}
