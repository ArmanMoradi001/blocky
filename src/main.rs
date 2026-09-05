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
    // let b1 = Block::new(0, "This is our first block in this blockchain".to_string().into_bytes());
    // b1.print_block();

    // let mut blockchain = BlockChain::new();

    // let previous_hash = blockchain.last_block().hash();
    // blockchain.create_block(1, previous_hash);

    // let previous_hash = blockchain.last_block().hash();
    // blockchain.create_block(2, previous_hash);

    // let previous_hash = blockchain.last_block().hash();
    // blockchain.create_block(3, previous_hash);
    // blockchain.print_blockchain();

    // let result = blockchain.block_search(BlockSearch::SearchByNonce(5));
    // get_block_search_result(result);

    let tx = Transaction::new(
        "sender".as_bytes().to_vec(),
        "reciepent".as_bytes().to_vec(),
        100,
    );
    println!("tx before serialization {}", tx);

    let tx_bin = tx.serialization();
    println!("tx after serialization {:?}", tx_bin);

    let tx_de = Transaction::deserialization(tx_bin);
    println!("tx after deserialization {}", tx_de);
}
