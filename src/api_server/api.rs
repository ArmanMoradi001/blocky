use crate::blockchain::blockchain::{BlockChain, Serialization};
use crate::blockchain::transaction::Transaction as BlockchainTransaction;
use crate::wallet::wallet::Wallet;
use actix_web::{App, HttpResponse, HttpServer, web};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::{fmt::Debug, sync::{Arc, Mutex}};

//http:://localhost:5000/amount/0x12345
#[derive(Serialize)]
struct QueryAmount {
    amount: i64,
}

#[derive(Clone, Debug)]
pub struct ApiServer {
    port: u16,

    cache: Arc<Mutex<HashMap<String, BlockChain>>>,
}

#[derive(Deserialize, Debug)]
pub struct TransactionRequest {
    pub private_key: String,
    pub public_key: String,
    pub blockchain_address: String,
    pub recipient_address: String,
    pub amount: String,
}

// Keep old name as alias so existing clients keep working.
pub type Transaction = TransactionRequest;

#[derive(Deserialize, Debug)]
pub struct FaucetRequest {
    pub recipient_address: String,
    pub amount: String,
}

#[derive(Serialize, Debug)]
pub struct TransferResponse {
    pub status: String,
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
    pub pending_in_pool: usize,
    pub note: String,
}

/// Human-readable transaction for API responses. The chain stores addresses
/// as raw bytes; the webpage needs plain strings.
#[derive(Serialize, Debug)]
pub struct TransactionView {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
}

impl From<&BlockchainTransaction> for TransactionView {
    fn from(tx: &BlockchainTransaction) -> Self {
        TransactionView {
            sender: String::from_utf8_lossy(&tx.sender_address).into_owned(),
            recipient: String::from_utf8_lossy(&tx.reciever_address).into_owned(),
            amount: tx.value,
        }
    }
}

/// Human-readable block for API responses (hashes as hex, txs decoded).
#[derive(Serialize, Debug)]
pub struct BlockView {
    pub index: usize,
    pub nonce: i32,
    pub previous_hash: String,
    pub hash: String,
    pub timestamp: u128,
    pub transactions: Vec<TransactionView>,
}

impl ApiServer {
    pub fn new(port: u16) -> Self {
        let cache = Arc::new(Mutex::new(HashMap::new()));
        let mut api_server = ApiServer { port, cache };

        let wallet_miner = Wallet::new();
        {
            /*
            get the lock of Mutex will return Result<ApiServer, None>, if we get lock ok,
            we need to use unwrap to get the ApiServer from Result

            The lock will cause reference to mutex, reference to the ApiServer,
            need to release the lock before returning the api_server,
            no unlock method, only way to unlock the mutex is let it go out of its scope
            */
            let mut unlock_cache = api_server.cache.lock().unwrap();
            unlock_cache.insert(
                "blockchain".to_string(),
                BlockChain::new(wallet_miner.address_str()),
            );
        }

        return api_server;
    }

    pub async fn get_amount(
        data: web::Data<Arc<ApiServer>>,
        path: web::Path<String>,
    ) -> HttpResponse {
        let address = path.into_inner();
        let api_server = data.get_ref();
        let unlock_cache = api_server.cache.lock().unwrap();
        let block_chain = unlock_cache.get("blockchain").unwrap();
        let amount = block_chain.calculate_total_amount(address);
        let amount_return = QueryAmount { amount };

        HttpResponse::Ok().json(amount_return)
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

    async fn get_wallet() -> HttpResponse {
        HttpResponse::Ok().content_type("text/html").body(
            r##"
            <!DOCTYPE html>
            <html lang="en">
            <head>
               <meta charset="UTF-8"/>
               <title>Wallet</title>
               <script src="https://ajax.googleapis.com/ajax/libs/jquery/3.7.1/jquery.min.js"></script>
               <script>
                  $(function(){
                    function show_status(msg, is_error) {
                        const el = $("#status");
                        el.text(msg);
                        el.css("color", is_error ? "red" : "green");
                    }
                    function server_error(xhr) {
                        // Actix returns JSON-encoded strings, e.g. "\"insufficient ...\"".
                        try {
                            const body = JSON.parse(xhr.responseText);
                            return typeof body === "string" ? body : JSON.stringify(body);
                        } catch (e) {
                            return xhr.responseText || xhr.statusText || "request failed";
                        }
                    }

                    // 1. Create a fresh wallet for this page load.
                    $.ajax({
                        url: "/get_wallet",
                        method: "GET",
                        success: function(response) {
                            console.log(response);
                            $("#public_key").val(response['public_key'])
                            $("#private_key").val(response['private_key'])
                            $("#blockchain_address").val(response['blockchain_address'])
                            reload_amount()
                        },
                        error: function(xhr) {
                            console.log(xhr)
                            show_status("Failed to create wallet: " + server_error(xhr), true)
                        }
                    })

                    $("#send_money").click(function(){
                        const recipient = $("#recipient_address").val().trim()
                        const amount = $("#send_amount").val().trim()
                        if (!recipient) { show_status("Recipient address must not be empty", true); return }
                        if (!amount) { show_status("Amount must not be empty", true); return }
                        if (!confirm('Are you ready to send ' + amount + ' to ' + recipient + '?')) {
                            show_status('Transaction cancelled', true)
                            return
                        }

                        const transaction = {
                            'private_key': $("#private_key").val(),
                            'blockchain_address': $("#blockchain_address").val(),
                            'public_key': $("#public_key").val(),
                            'recipient_address': recipient,
                            'amount': amount,
                        }

                        $.ajax({
                            url: "/transaction",
                            method: "POST",
                            contentType: "application/json",
                            data: JSON.stringify(transaction),
                            success: function(response) {
                                console.log(response)
                                show_status('Queued: ' + response.amount + ' -> ' + response.recipient + '. Press Mine to confirm.', false)
                            },
                            error: function(xhr) {
                                console.error(xhr)
                                show_status('Transaction failed (' + xhr.status + '): ' + server_error(xhr), true)
                            }
                        })
                    })

                    // 2. Faucet: new wallets start at 0, so mint test coins first.
                    $("#get_faucet").click(function(){
                        const address = $("#blockchain_address").val().trim()
                        const amount = $("#faucet_amount").val().trim() || "100"
                        if (!address) { show_status("Wallet not loaded yet", true); return }
                        $.ajax({
                            url: "/faucet",
                            method: "POST",
                            contentType: "application/json",
                            data: JSON.stringify({recipient_address: address, amount: amount}),
                            success: function(response) {
                                console.log(response)
                                show_status('Faucet queued ' + response.amount + '. Press Mine to confirm.', false)
                            },
                            error: function(xhr) {
                                console.error(xhr)
                                show_status('Faucet failed (' + xhr.status + '): ' + server_error(xhr), true)
                            }
                        })
                    })

                    // 3. Mining confirms queued (faucet + transfer) transactions.
                    $("#mine").click(function(){
                        $.ajax({
                            url: "/mining",
                            method: "GET",
                            success: function(response) {
                                console.log(response)
                                show_status('Mined a block. Balance updated below.', false)
                                reload_amount()
                            },
                            error: function(xhr) {
                                console.error(xhr)
                                show_status('Mining failed (' + xhr.status + '): ' + server_error(xhr), true)
                            }
                        })
                    })

                    function reload_amount() {
                        const address = $("#blockchain_address").val()
                        if (!address || !address.trim()) {
                            return
                        }
                        const url = "/amount/" + address.trim()
                        $.ajax({
                            url: url,
                            type: "GET",
                            success: function(response) {
                                let amount = response["amount"]
                                $("#input_amount").text(amount)
                                console.log(amount)
                            },
                            error: function(xhr) {
                                console.error(xhr)
                            }
                        })
                    }

                    $("#refresh_wallet").click(function(){
                        reload_amount()
                    })

                    setInterval(reload_amount, 3000)
                  })
               </script>
            </head>
            <body>
                <div>
                  <h1>Wallet</h1>
                   <div>Balance: <span id="input_amount">0</span></div>
                   <div id="status"></div>
                   <button id="refresh_wallet">Refresh Wallet</button>
                   <p>Public Key</p>
                   <textarea id="public_key" rows="2" cols="100">
                   </textarea>

                   <p>Private Key</p>
                   <textarea id="private_key" rows="1" cols="100">
                   </textarea>

                    <p>Blockchain address</p>
                   <textarea id="blockchain_address" rows="1" cols="100">
                   </textarea>
                </div>

                <div>
                    <h1>Get Test Coins (faucet)</h1>
                    <div>
                        Amount: <input id="faucet_amount" type="text" value="100"/>
                        <button id="get_faucet">Get Coins</button>
                    </div>
                </div>

                <div>
                    <h1>Mine</h1>
                    <div>
                        <button id="mine">Mine a Block (confirm pending)</button>
                    </div>
                </div>

                <div>
                    <h1>Send Money</h1>
                    <div>
                        Address: <input id="recipient_address" size="100" type="text"></input>
                        <br>
                        Amount: <input id="send_amount" type="text"/>
                        <br>
                        <button id="send_money">Send</button>
                    </div>
                </div>

            </body>
            </html>
            "##
        )
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

    pub async fn run(&self) {
        /*
        the server will create 4 worker threads to handle requests,
        share data,
        use Arc automatic resource counter to wrap the api server
        */
        let api = Arc::new(self.clone());

        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(api.clone()))
                .wrap(actix_web::middleware::Logger::default())
                .route("/", web::get().to(Self::get_index_handler))
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
        });

        println!("Server running on port:{}", self.port);
        server
            .bind(("0.0.0.0", self.port))
            .unwrap()
            .run()
            .await
            .expect("Error running the server");
    }
}