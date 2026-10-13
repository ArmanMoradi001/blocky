
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use log::info;
use tokio;

use crate::api_server::api::ApiServer;

use super::membership::{Membership, MembershipConfig};
use super::sync;

/// One entry per node: same default HTTP port, distinct loopback IP.
/// Each node keeps its own chain, converged via Nakamoto longest-valid-chain.
/// Membership gossip runs on UDP at `http_port + UDP_PORT_OFFSET`.
const NODES: [(&str, u16); 3] = [
    ("127.0.0.1", 5000),
    ("127.0.0.2", 5000),
    ("127.0.0.3", 5000),
];

pub(crate) const UDP_PORT_OFFSET: u16 = 10_000;

/// How often each node pulls the tallest peer chain + mempools.
const SYNC_INTERVAL: Duration = Duration::from_secs(2);

fn udp_addr(http_host: &str, http_port: u16) -> SocketAddr {
    format!("{http_host}:{}", http_port + UDP_PORT_OFFSET)
        .parse()
        .expect("invalid node address")
}

pub async fn run_server() {
    // Every node knows every other node's UDP address as a join seed.
    let udp_addrs: Vec<SocketAddr> = NODES.iter().map(|(h, p)| udp_addr(h, *p)).collect();

    let mut handles = vec![];

    for (i, (host, port)) in NODES.into_iter().enumerate() {
        let self_udp = udp_addrs[i];
        let seeds: Vec<SocketAddr> = udp_addrs
            .iter()
            .cloned()
            .filter(|a| *a != self_udp)
            .collect();

        let handle = tokio::spawn(async move {
            run_node(host, port, self_udp, seeds).await;
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}

/// One node: HTTP API + UDP membership gossip + Nakamoto sync loop.
async fn run_node(http_host: &str, http_port: u16, udp_addr: SocketAddr, seeds: Vec<SocketAddr>) {
    let membership = Membership::bind(udp_addr, seeds, MembershipConfig::default()).await;
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
