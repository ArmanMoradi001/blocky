mod api_server;
mod blockchain;
mod crypto;
mod wallet;
mod network;
use core::hash;
use std::result;

use tokio;
use actix_web::{App, HttpServer, Responder, get};
use api_server::api::ApiServer;
use blockchain::blockchain::Serialization;
use blockchain::blockchain::{Block, BlockChain, BlockSearch, BlockSearchResult};
use blockchain::transaction::Transaction;
// use crypto::generate_and_sign;
use actix_web::middleware::Logger;
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

// // #[actix_web::main]
// #[tokio::main]
#[tokio::main]
async fn main() {
    env_logger::init();
    network::network::run_server().await;
}
