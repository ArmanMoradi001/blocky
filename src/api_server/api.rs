use crate::blockchain::blockchain::BlockChain;
use crate::wallet::wallet;
use actix_web::{App, HttpResponse, HttpServer, web};
use log::{debug, error, info, trace, warn};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct ApiServer {
    pub port: u16,
    pub cache: HashMap<String, BlockChain>,
}

impl ApiServer {
    pub fn new(port: u16) -> Self {
        let mut apiserver = ApiServer {
            port,
            cache: HashMap::new(),
        };
        let miner = wallet::Wallet::new();
        apiserver.cache.insert(
            "blockchain".to_string(),
            BlockChain::new(miner.address_str()),
        );
        return apiserver;
    }

    pub async fn get_index(&self) -> HttpResponse {
        let blockchain = self.cache.get("blockchain").unwrap();
        let first_block = blockchain[0].clone();
        let block_json = serde_json::to_string(&first_block).unwrap();
        println!("block_json: {}", block_json);
        HttpResponse::Ok().json(block_json)
    }

    pub async fn get_index_handler(data: web::Data<Arc<ApiServer>>) -> HttpResponse {
        // Log entry into the handler
        info!("Received request at '/' endpoint.");
        // Log the received data
        debug!("Handler received ApiServer data: {:?}", data);

        data.get_ref().get_index().await
    }

    pub async fn run_server(&self) {
        let api = Arc::new(self.clone());
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(api.clone()))
                .wrap(actix_web::middleware::Logger::default())
                .route("/", web::get().to(Self::get_index_handler))
        });

        println!("Server running on port: {}", self.port);
        server
            .bind(("0.0.0.0", self.port as u16))
            .unwrap()
            .run()
            .await
            .expect("Error running the server");
    }
}
