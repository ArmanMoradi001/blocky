mod blockchain;

use blockchain::{Block, BlockChain};
fn main() {
    // let b1 = Block::new(0, "This is our first block in this blockchain".to_string().into_bytes());
    // b1.print_block();

    let mut blockchain = BlockChain::new();

    let previous_hash = blockchain.last_block().hash();
    blockchain.create_block(1, previous_hash);

    let previous_hash = blockchain.last_block().hash();
    blockchain.create_block(2, previous_hash);

    let previous_hash = blockchain.last_block().hash();
    blockchain.create_block(3, previous_hash);
    blockchain.print_blockchain();
}
