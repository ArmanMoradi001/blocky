
use tokio;
use crate::api_server::api::ApiServer;

pub async fn run_server() {
    let ports = vec![5000, 5001, 5002];
    let mut handles = vec![];

    for port in ports {
        let server = ApiServer::new(port);

        let handle = tokio::spawn(async move {
            server.run().await;
        });

        handles.push(handle);
    }

    for handle in handles {
        handle.await.unwrap();
    }
}
