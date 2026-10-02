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

#[derive(Deserialize, Debug)]
pub struct Transaction {
    pub private_key: String,
    pub blockchain_address: String,
    pub public_key: String,
    pub recipient_address: String,
    pub amount: String,
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


       pub async fn get_wallet_handler() -> HttpResponse {
        let wallet_user = wallet::Wallet::new();
        let wallet_data = wallet_user.get_wallet_data();
        HttpResponse::Ok().json(wallet_data)
    }

    pub async fn run_server(&self) {
        let api = Arc::new(self.clone());
        let server = HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(api.clone()))
                .wrap(actix_web::middleware::Logger::default())
                .route("/", web::get().to(Self::get_index_handler))
                .route("/wallet", web::get().to(Self::get_wallet))
                .route("/get_wallet", web::get().to(Self::get_wallet_handler))
        });

        println!("Server running on port: {}", self.port);
        server
            .bind(("0.0.0.0", self.port as u16))
            .unwrap()
            .run()
            .await
            .expect("Error running the server");
    }

    pub async fn get_wallet() -> HttpResponse {
        HttpResponse::Ok().content_type("text/html").body(
            r#"
            <!DOCTYPE html>
            <html lang="en">
            <head>
                <meta charset="UTF-8">
                <title>Wallet</title>
                <script src="https://ajax.googleapis.com/ajax/libs/jquery/3.7.1/jquery.min.js">
                </script> 

                <script> 
                $(function() {
                    $.ajax({
                        url: "/get_wallet",
                        method: "GET",
                        success: function(response) {
                            console.log(response);
                            $("\#public_key").val(response['public_key']);
                            $("\#private_key").val(response['private_key']);
                            $("\#blockchain_address").val(response['blockchain_address']);
                        },
                        error: function(error) {
                            console.info(error);
                        }
                    })

                    $("\#send_money").click(function() {
                        let confirm_text = 'Are you ready to send the given amount?'
                        let confirm_result = confirm(confirm_text);
                        if (!confirm_result) {
                            alert('Transaction cancelled');
                            return;
                        }
                        let transaction = {
                            'private_key': $("\#private_key").val(),
                            'blockchain_address': $("\#blockchain_address").val(),
                            'public_key': $("\#public_key").val(),
                            'recipient_address': $("\#recipient_address").val(),
                            'amount': $("\#send_amount").val(),
                        }
                        
                        $.ajax({
                            url: "/transaction",
                            method: "POST",
                            contentType: "application/json",
                            data: JSON.stringify(transaction),
                            success: function(response) {
                                console.log(response);
                                alert('success');
                            },
                            error: function(error) {
                                console.error(error);
                                alert('error');
                            }
                        })
                    })
                })
                </script>
            </head>
            <body>
                <div>
                   <h1>Wallet</h1>
                   <div id="input_amount">0</div>
                   <button id="refresh_wallet">Refresh Wallet</button>
                   <p>Publick Key</p>
                   <textarea id="public_key" row="2" cols="100">
                   </textarea>

                   <p>Private Key</p>
                   <textarea id="private_key" row="1", cols="100">
                   </textarea>

                    <p>Blockchain address</p>
                   <textarea id="blockchain_address" row="1", cols="100">
                   </textarea>
                </div>

                </div>
                    <h1>Send Money</h1>
                    <div>
                        Address: <input id="recipient_address" size="100" type="text" />
                        <br>
                        Amount: <input id="send_amount" type="text" />
                        <br>
                        <button id="send_money">Send</button>
                    </div>
                </div>
            </body>
            </html>
            "#,
        )
    }
    pub async fn get_transaction_handler(transaction: web::Json<Transaction>) -> HttpResponse {
        // let blockchain = self.cache.get("blockchain").unwrap();
        // let transaction = blockchain.create_transaction(transaction.private_key, transaction.blockchain_address, transaction.recipient_address, transaction.amount);
        // HttpResponse::Ok().json(transaction)
        debug!("receive json info: {:?}", transaction.into_inner());
        HttpResponse::Ok().json("transaction")
    }

}
