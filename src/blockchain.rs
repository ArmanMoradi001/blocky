use sha2::{Digest, Sha256};
use std::{iter::chain, time::SystemTime};

use crate::transaction::Transaction;

pub trait Serialization<T> {
    fn serialization(&self) -> Vec<u8>;

    fn deserialization(bytes: Vec<u8>) -> T;
}

pub enum BlockSearch {
    SearchByIndex(usize),
    SearchByNonce(i32),
    SearchByPreviousHash(Vec<u8>),
    SearchByTimeStamp(u128),
    SearchByTransactions(Vec<u8>),
    SearchByHash(Vec<u8>),
}

pub enum BlockSearchResult<'a> {
    Success(&'a Block),
    FailOfEmptyBlock,
    FailOfIndex(usize),
    FailOfNonce(i32),
    FailOfPreviousHash(Vec<u8>),
    FailOfTimeStamp(u128),
    FailOfTransactions(Vec<u8>),
    FailOfHash(Vec<u8>),
}

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
        let mut block = Block::new(nonce, previous_hash);
        for tx in self.transactions_pool.iter(){
            block.transactions.push(tx.clone());
        }
        self.transactions_pool.clear();
        self.chain.push(block);
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

    pub fn block_search(&self, search: BlockSearch) -> BlockSearchResult {
        for block in self.chain.iter() {
            match search {
                BlockSearch::SearchByIndex(index) => {
                    if index
                        == self
                            .chain
                            .iter()
                            .position(|b| std::ptr::eq(b, block))
                            .unwrap()
                    {
                        return BlockSearchResult::Success(block);
                    }
                }

                BlockSearch::SearchByNonce(nonce) => {
                    if nonce == block.nonce {
                        return BlockSearchResult::Success(block);
                    }
                }

                BlockSearch::SearchByPreviousHash(ref phash) => {
                    if block.previous_hash == *phash {
                        return BlockSearchResult::Success(block);
                    }
                }

                BlockSearch::SearchByTimeStamp(time_stamp) => {
                    if block.time_stamp == time_stamp {
                        return BlockSearchResult::Success(block);
                    }
                }

                BlockSearch::SearchByTransactions(ref tx) => {
                    if block.transactions.contains(tx) {
                        return BlockSearchResult::Success(block);
                    }
                }

                BlockSearch::SearchByHash(ref hash) => {
                    if block.hash() == *hash {
                        return BlockSearchResult::Success(block);
                    }
                }
            }
        }

        if self.chain.is_empty() {
            return BlockSearchResult::FailOfEmptyBlock;
        }

        match search {
            BlockSearch::SearchByIndex(index) => BlockSearchResult::FailOfIndex(index),
            BlockSearch::SearchByNonce(nonce) => BlockSearchResult::FailOfNonce(nonce),
            BlockSearch::SearchByPreviousHash(phash) => {
                BlockSearchResult::FailOfPreviousHash(phash)
            }
            BlockSearch::SearchByTimeStamp(time_stamp) => {
                BlockSearchResult::FailOfTimeStamp(time_stamp)
            }
            BlockSearch::SearchByTransactions(tx) => BlockSearchResult::FailOfTransactions(tx),
            BlockSearch::SearchByHash(hash) => BlockSearchResult::FailOfHash(hash),
        }
    }


    pub fn add_transaction(&mut self, tx: &impl Serialization<Transaction> ){
        for tx_in_pool in self.transactions_pool.iter(){
            if tx_in_pool == tx.serialization(){
                return;;
            }
        }

        self.transactions_pool.push(tx.serialization());
    }
}
