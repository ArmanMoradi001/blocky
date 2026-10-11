
use crate::api_server::api::ApiServer;
use tokio;

/// One entry per node: same default port, distinct loopback IP.
/// Each node keeps its own independent in-memory chain.
const NODES: [(&str, u16); 3] = [
    ("127.0.0.1", 5000),
    ("127.0.0.2", 5000),
    ("127.0.0.3", 5000),
];

pub async fn run_server() {
    let mut handles = vec![];

    for (host, port) in NODES {
        let server = ApiServer::new(host, port);

        let handle = tokio::spawn(async move {
            server.run().await;
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
