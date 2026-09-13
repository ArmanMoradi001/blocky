mod blockchain;
mod transaction;

use core::hash;
use std::result;

use blockchain::{Block, BlockChain, BlockSearch, BlockSearchResult};
use transaction::Transaction;

use crate::blockchain::Serialization;

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
    let mut block_chain = BlockChain::new();
    block_chain.print_blockchain();
}
