use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use log::{info, warn};
use tokio;

use crate::api_server::api::ApiServer;

use super::membership::{Membership, MembershipConfig};
use super::sync;

/// Default local-dev cluster: same HTTP port on distinct loopback IPs.
/// Each node keeps its own chain, converged via Nakamoto longest-valid-chain.
/// Membership gossip runs on UDP at `http_port + UDP_PORT_OFFSET`.
const DEFAULT_NODES: [(&str, u16); 3] = [
    ("127.0.0.1", 5000),
    ("127.0.0.2", 5000),
    ("127.0.0.3", 5000),
];

pub(crate) const UDP_PORT_OFFSET: u16 = 10_000;

/// How often each node pulls the tallest peer chain + mempools.
const SYNC_INTERVAL: Duration = Duration::from_secs(2);

/// Resolve `host:port` to a `SocketAddr`, following Docker/Compose DNS names.
/// Plain IPs parse directly; anything else goes through `lookup_host` and
/// takes the first result.
async fn resolve_one(s: &str) -> SocketAddr {
    if let Ok(addr) = s.parse::<SocketAddr>() {
        return addr;
    }
    let mut addrs = tokio::net::lookup_host(s)
        .await
        .unwrap_or_else(|_| panic!("invalid node address: {s}"));
    addrs
        .next()
        .unwrap_or_else(|| panic!("DNS returned no records for: {s}"))
}

async fn resolve_many(list: &str) -> Vec<SocketAddr> {
    let mut out = vec![];
    for part in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        out.push(resolve_one(part).await);
    }
    out
}

/// Multi-node topology: `BLOCKY_NODES="host:port,host:port,..."`
/// (default: the 3 loopback nodes). Hosts may be DNS names.
async fn multi_node_topology() -> Vec<(String, u16, SocketAddr)> {
    let raw = std::env::var("BLOCKY_NODES").unwrap_or_default();
    let http_addrs: Vec<String> = if raw.trim().is_empty() {
        DEFAULT_NODES
            .iter()
            .map(|(h, p)| format!("{h}:{p}"))
            .collect()
    } else {
        raw.split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    let mut topo = vec![];
    for http in http_addrs {
        let resolved = resolve_one(&http).await;
        let port = resolved.port();
        let (host_str, _) = http
            .rsplit_once(':')
            .expect("BLOCKY_NODES entries must be host:port");
        let _ = port; // resolved port is authoritative for UDP math
        let udp_port = resolved
            .port()
            .checked_add(UDP_PORT_OFFSET)
            .expect("UDP port overflow");
        let udp_advertise = SocketAddr::new(resolved.ip(), udp_port);
        let _ = host_str;
        topo.push((http, resolved.port(), udp_advertise));
    }
    topo
}

pub async fn run_server() {
    // Single-node (Docker/Compose) mode: BLOCKY_UDP_ADVERTISE is set.
    // Each container runs exactly one node:
    //   BLOCKY_HTTP_HOST      bind host for HTTP (default 0.0.0.0)
    //   BLOCKY_HTTP_PORT      HTTP port (default 5000)
    //   BLOCKY_UDP_BIND       UDP bind addr (default 0.0.0.0:<http+offset>)
    //   BLOCKY_UDP_ADVERTISE  routable self addr, e.g. node1:15000 (required)
    //   BLOCKY_SEEDS          comma-separated peer UDP addrs (may be empty)
    if std::env::var("BLOCKY_UDP_ADVERTISE")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
    {
        run_single_node().await;
        return;
    }
    run_multi_node().await;
}

async fn run_single_node() {
    let http_host = std::env::var("BLOCKY_HTTP_HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let http_port: u16 = std::env::var("BLOCKY_HTTP_PORT")
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(5000);
    let udp_advertise = resolve_one(
        &std::env::var("BLOCKY_UDP_ADVERTISE").expect("BLOCKY_UDP_ADVERTISE must be set"),
    )
    .await;
    let default_udp_bind = format!("0.0.0.0:{}", http_port + UDP_PORT_OFFSET);
    let udp_bind_str = std::env::var("BLOCKY_UDP_BIND").unwrap_or(default_udp_bind);
    let udp_bind = resolve_one(&udp_bind_str).await;
    let seeds = std::env::var("BLOCKY_SEEDS").unwrap_or_default();
    let seeds = resolve_many(&seeds).await;

    if udp_advertise.port() != http_port + UDP_PORT_OFFSET {
        warn!(
            "network: UDP advertise port {} != HTTP port {http_port} + offset {UDP_PORT_OFFSET}; \
             sync derives HTTP peers as udp_port - offset, so peers will dial the wrong HTTP port",
            udp_advertise.port()
        );
    }
    info!(
        "network: single node http={http_host}:{http_port} udp_bind={udp_bind} udp_advertise={udp_advertise} seeds={seeds:?}"
    );

    run_node(&http_host, http_port, udp_bind, udp_advertise, seeds).await;
}

async fn run_multi_node() {
    // Optional: BLOCKY_BIND_HOST=0.0.0.0 binds all sockets to 0.0.0.0
    // (needed for Docker port-mapping) while advertising the NODES IPs.
    // Unset = bind to the advertise IP (legacy local-dev behaviour).
    let bind_host = std::env::var("BLOCKY_BIND_HOST")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let topo = multi_node_topology().await;
    let udp_advertises: Vec<SocketAddr> = topo.iter().map(|(_, _, u)| *u).collect();

    let mut handles = vec![];
    for (i, (http_str, http_port, self_udp_advertise)) in topo.into_iter().enumerate() {
        let seeds: Vec<SocketAddr> = udp_advertises
            .iter()
            .cloned()
            .filter(|a| *a != self_udp_advertise)
            .collect();
        let bind_host = bind_host.clone();
        let handle = tokio::spawn(async move {
            // http_str is "host:port"; split for the HTTP bind host.
            let (cfg_host, _) = http_str.rsplit_once(':').unwrap();
            let http_bind_host = bind_host.clone().unwrap_or_else(|| cfg_host.to_string());
            let udp_bind = if http_bind_host == cfg_host {
                self_udp_advertise
            } else {
                let ip = resolve_one(&format!("{http_bind_host}:0")).await.ip();
                SocketAddr::new(ip, self_udp_advertise.port())
            };
            let _ = i;
            run_node(
                &http_bind_host,
                http_port,
                udp_bind,
                self_udp_advertise,
                seeds,
            )
            .await;
        });
        handles.push(handle);
    }

    // Wait for all nodes in a separate task so Ctrl+C (SIGINT) can
    // interrupt the wait. Without this, the infinite membership/sync loops
    // keep the process alive forever and Ctrl+C appears to do nothing.
    let mut waiter = tokio::spawn(async move {
        for handle in handles {
            let _ = handle.await;
        }
    });

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("network: SIGINT received, shutting down nodes");
            waiter.abort();
        }
        _ = &mut waiter => {}
    }
}

/// One node: HTTP API + UDP membership gossip + Nakamoto sync loop.
async fn run_node(
    http_host: &str,
    http_port: u16,
    udp_bind: SocketAddr,
    udp_advertise: SocketAddr,
    seeds: Vec<SocketAddr>,
) {
    let membership =
        Membership::bind(udp_bind, udp_advertise, seeds, MembershipConfig::default()).await;
    let mut server = ApiServer::new(http_host, http_port);
    server.set_membership(membership.clone());
    let api = Arc::new(server);
    let sync_api = api.clone();
    let sync_membership = membership.clone();

    let sync_task = tokio::spawn(async move {
        // Let SWIM discover peers + HTTP bind before first pull.
        tokio::time::sleep(Duration::from_secs(3)).await;
        loop {
            let udp_peers = sync_membership.alive_addrs().await;
            let http_peers: Vec<SocketAddr> = udp_peers
                .into_iter()
                .filter_map(|u| sync::http_addr_from_udp(u, UDP_PORT_OFFSET))
                .collect();
            let summary = sync::sync_once(&sync_api.cache(), http_peers).await;
            info!(
                "sync: {}:{} {}",
                sync_api.host_str(),
                sync_api.port(),
                summary
            );
            tokio::time::sleep(SYNC_INTERVAL).await;
        }
    });

    tokio::join!(
        membership.run(),
        async {
            api.run().await;
        },
        async {
            let _ = sync_task.await;
        }
    );
}
