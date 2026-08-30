use std::{iter::chain, time::SystemTime};

#[derive(Debug)]
pub struct Block {
    nonce: i32,
    previous_hash: Vec<u8>,
    time_stamp: u128,
    transactions: Vec<Vec<u8>>,
}

impl Block {
    pub fn new(nonce: i32, previous_hash: Vec<u8>) -> Self {
        let time_now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        Block {
            nonce,
            previous_hash,
            time_stamp: time_now.as_nanos(),
            transactions: Vec::<Vec<u8>>::new(),
        }
    }

    pub fn print_block(&self) {
        println!("This is previous hash: {:?}", self.previous_hash);
        println!("This is nonce: {}", self.nonce);
        println!("This is transactions vector: {:?}", self.transactions);
        println!("This is time stamp: {:x}", self.time_stamp)
    }
}

#[derive(Debug)]

pub struct BlockChain {
    transactions_pool: Vec<Vec<u8>>,
    chain: Vec<Block>,
}

impl BlockChain {
    pub fn new() -> Self {
        let mut bc = BlockChain {
            transactions_pool: Vec::<Vec<u8>>::new(),
            chain: Vec::<Block>::new(),
        };

        bc.create_block(0, "This is the very block hash".to_string().into_bytes());
        bc
    }

    fn create_block(&mut self, nonce: i32, previous_hash: Vec<u8>) {
        let b = Block::new(nonce, previous_hash);
        self.chain.push(b);
    }

    pub fn print_blockchain(&self) {
        for (idx, block) in self.chain.iter().enumerate() {
            println!("{} The chain {} {}", "=".repeat(25), idx, "=".repeat(25));
            block.print_block();
        }
        println!("{}", "*".repeat(25));
    }
}

fn main() {
    // let b1 = Block::new(0, "This is our first block in this blockchain".to_string().into_bytes());
    // b1.print_block();

    let mut bc1 = BlockChain::new();

    bc1.create_block(1, "This is the hash 1".to_string().into_bytes());
    bc1.create_block(2, "This is the hash 2".to_string().into_bytes());
    bc1.create_block(3, "This is the hash 3".to_string().into_bytes());
    bc1.print_blockchain();
}
