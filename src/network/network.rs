
use std::net::SocketAddr;

use tokio;

use crate::api_server::api::ApiServer;

use super::membership::{Membership, MembershipConfig};

/// One entry per node: same default HTTP port, distinct loopback IP.
/// Each node keeps its own independent in-memory chain.
/// Membership gossip runs on UDP at `http_port + UDP_PORT_OFFSET`.
const NODES: [(&str, u16); 3] = [
    ("127.0.0.1", 5000),
    ("127.0.0.2", 5000),
    ("127.0.0.3", 5000),
];

const UDP_PORT_OFFSET: u16 = 10_000;

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

/// One node: HTTP API + UDP membership gossip side by side.
async fn run_node(http_host: &str, http_port: u16, udp_addr: SocketAddr, seeds: Vec<SocketAddr>) {
    let membership = Membership::bind(udp_addr, seeds, MembershipConfig::default()).await;
    let server = ApiServer::new(http_host, http_port);

    tokio::join!(membership.run(), async {
        server.run().await;
    });
}
