mod blockchain;
mod crypto;
mod wallet;
use core::hash;
use std::result;

use blockchain::blockchain::{Block, BlockChain, BlockSearch, BlockSearchResult};
use blockchain::transaction::Transaction;

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
fn main() {
    let miner = Wallet::new();

    let mut blockchain = BlockChain::new(miner.address_str());

    let wallet_a = Wallet::new();
    let wallet_b = Wallet::new();

    let tx_a_b = wallet_a.sign_transaction(&wallet_b.address_str(), 100);

    println!(
        "the transactio state is: {}",
        blockchain.add_transaction(&tx_a_b)
    );
}
