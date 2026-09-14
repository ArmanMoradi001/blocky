mod blockchain;
mod transaction;
mod crypto;

use core::hash;
use std::result;

use blockchain::{Block, BlockChain, BlockSearch, BlockSearchResult};
use transaction::Transaction;

use crate::blockchain::Serialization;
use crate::crypto::generate_and_sign;

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
    let my_blockchain_addr = "my blockchain address";
    let mut block_chain = BlockChain::new(my_blockchain_addr.into());
    block_chain.print_blockchain();

    block_chain.add_transaction(&Transaction::new("A".into(), "B".into(), 1));
    block_chain.mining();
    block_chain.print_blockchain();

    block_chain.add_transaction(&Transaction::new("C".into(), "D".into(), 1));
    block_chain.add_transaction(&Transaction::new("X".into(), "Y".into(), 1));
    block_chain.mining();
    block_chain.print_blockchain();

    println!(
        "value for miner: {}",
        block_chain.calculate_total_amount(my_blockchain_addr.to_string())
    );
    println!(
        "value for C: {}",
        block_chain.calculate_total_amount("C".to_string())
    );
    println!(
        "value for D: {}",
        block_chain.calculate_total_amount("D".to_string())
    );

    generate_and_sign(b"This is my message");
    
}

