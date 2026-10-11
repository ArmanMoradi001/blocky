//! HTTP API server: routing + handlers only.
//!
//! Layout:
//! - `models.rs` — request/response DTOs (`TransactionRequest`, views, …)
//! - `views.rs` + `templates/` + `static/` — wallet dashboard HTML/CSS/JS
//! - this file — `ApiServer` state, endpoint handlers, route table.

use std::{
    collections::HashMap,
    fmt::Debug,
    sync::{Arc, Mutex},
};

use actix_web::{App, HttpResponse, HttpServer, web};
use log::{debug, info};

use crate::blockchain::blockchain::{BlockChain, Serialization};
use crate::blockchain::transaction::Transaction as BlockchainTransaction;
use crate::wallet::wallet::Wallet;

use super::models::{
    BlockView, FaucetRequest, QueryAmount, TransactionRequest, TransactionView, TransferResponse,
};
use super::views::render_wallet_page;

#[derive(Clone, Debug)]
pub struct ApiServer {
    host: String,
    port: u16,
    cache: Arc<Mutex<HashMap<String, BlockChain>>>,
}

impl ApiServer {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let api_server = ApiServer {
            host: host.into(),
            port,
            cache,
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
        let mut unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get_mut("blockchain").unwrap();
        if !block_chain.mining() {
            return HttpResponse::InternalServerError().json("mining fail");
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
                HttpResponse::Ok().json(serde_json::json!({
                    "status": "ok",
                    "recipient": recipient,
                    "amount": amount,
                    "pending_in_pool": block_chain.pending_count(),
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
        debug!("receive json info: {:?}", tx);

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

        // Rebuild the sender wallet from the supplied keys. Never panics:
        // mismatched/corrupt keys become a 400.
        let wallet = match Wallet::try_new_from(
            tx.public_key.trim(),
            tx.private_key.trim(),
            &sender,
        ) {
            Ok(w) => w,
            Err(e) => return HttpResponse::BadRequest().json(e),
        };
        let wallet_tx = wallet.sign_transaction(&recipient, amount);

        let api_server = data.get_ref();
        let mut unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get_mut("blockchain").unwrap();
        match block_chain.add_transaction(&wallet_tx) {
            Ok(()) => {
                info!("queued transaction {sender} -> {recipient} : {amount}");
                HttpResponse::Created().json(TransferResponse {
                    status: "pending".to_string(),
                    sender,
                    recipient,
                    amount,
                    pending_in_pool: block_chain.pending_count(),
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
            .route("/amount/{address}", web::get().to(Self::get_amount));
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
