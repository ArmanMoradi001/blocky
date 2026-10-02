mod api_server;
mod blockchain;
mod crypto;
mod wallet;
use core::hash;
use std::result;

use blockchain::blockchain::{Block, BlockChain, BlockSearch, BlockSearchResult};
use blockchain::transaction::Transaction;
use actix_web::{get, App, HttpServer, Responder};
use api_server::api::ApiServer;
use blockchain::blockchain::Serialization;
// use crypto::generate_and_sign;
use wallet::wallet::Wallet;

pub fn get_block_search_result(result: BlockSearchResult) {
    match result {
        BlockSearchResult::Success(block) => {
            println!("find given block {:?}", block);
        }

        BlockSearchResult::FailOfEmptyBlock => {
            println!("The chain was empty!");
        }

        BlockSearchResult::FailOfHash(hash) => {
            println!("Failed to find a block with given hash!")
        }
        BlockSearchResult::FailOfIndex(indx) => {
            println!("Failed to find a block with given index")
        }
        BlockSearchResult::FailOfNonce(nonce) => {
            println!("Failed to find a block with given nonce")
        }
        BlockSearchResult::FailOfPreviousHash(hash) => {
            println!("Failed to find a block with given hash")
        }
        BlockSearchResult::FailOfTimeStamp(time_stamp) => {
            println!("Failed to find a block with given time stamp")
        }
        BlockSearchResult::FailOfTransactions(tx) => {
            println!("Failed to find a block with given tx")
        }
    }
}

#[actix_web::main]
async fn main() {
    let server = ApiServer::new(5000);
    server.run_server().await;


    // let miner = Wallet::new();

    // let mut blockchain = BlockChain::new(miner.address_str());

    // let wallet_a = Wallet::new();
    // let wallet_b = Wallet::new();

    // let tx_a_b = wallet_a.sign_transaction(&wallet_b.address_str(), 100);

    // println!(
    //     "the transaction state is: {}",
    //     blockchain.add_transaction(&tx_a_b)
    // );
    // blockchain.mining();
    // blockchain.print_blockchain();

    // println!("The blcance for A is: {}", blockchain.calculate_total_amount(wallet_a.address_str()));
    // println!("The blcance for B is: {}", blockchain.calculate_total_amount(wallet_b.address_str()));
    // println!("The blcance for miner is: {}", blockchain.calculate_total_amount(miner.address_str()));
}


 
// #[get("/")]
// async fn hello() -> impl Responder {
//     "Hello, world!"
// }
 
// #[actix_web::main]
// async fn main() -> std::io::Result<()> {
//     HttpServer::new(|| App::new().service(hello))
//         .bind(("127.0.0.1", 8080))?
//         .run()
//         .await
// }