use sha2::{Digest, Sha256};
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

    pub fn hash(&self) -> Vec<u8> {
        let mut bin: Vec<u8> = vec![];
        bin.extend(self.nonce.to_be_bytes());
        bin.extend(self.previous_hash.clone());
        bin.extend(self.time_stamp.to_be_bytes());
        for tx in self.transactions.iter() {
            bin.extend(tx.clone());
        }
        let mut hasher = Sha256::new();
        hasher.update(bin);
        hasher.finalize().to_vec()
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

        bc.create_block(0, vec![0 as u8; 32]);
        bc
    }

    pub fn create_block(&mut self, nonce: i32, previous_hash: Vec<u8>) {
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

    pub fn last_block(&self) -> &Block {
        if self.chain.len() > 1 {
            return &self.chain[&self.chain.len() - 1];
        }

        &self.chain[0]
    }
}
