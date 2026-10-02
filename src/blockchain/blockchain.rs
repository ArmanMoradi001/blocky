use hex;
use p256::PublicKey;
use p256::ecdsa::signature;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cmp::PartialEq;
use std::ops::{AddAssign, Index};
use std::time::Instant;
use std::{iter::chain, time::SystemTime};

use super::transaction::Transaction;
use crate::wallet::wallet::{Transaction as WalletTransaction, Wallet};

pub trait Serialization<T> {
    fn serialization(&self) -> Vec<u8>;

    fn deserialization(bytes: Vec<u8>) -> T;
}

impl AddAssign<i32> for Block {
    fn add_assign(&mut self, rhs: i32) {
        self.nonce += rhs;
    }
}
impl PartialEq for Block {
    fn eq(&self, other: &Block) -> bool {
        self.hash() == other.hash()
    }
}

impl Index<usize> for BlockChain {
    type Output = Block;
    fn index(&self, idx: usize) -> &Self::Output {
        let res = self.chain.get(idx);
        match res {
            Some(block) => {
                return block;
            }
            None => {
                panic!("index out of range for the chain");
            }
        }
    }
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

#[derive(Debug, Clone, Serialize)]
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
        println!("timestamp: {:x}", self.time_stamp);
        //format integer value
        println!("nonce:      {},", self.nonce);
        //let compiler decide how to print vector
        //actually it is called trait, go to it later
        println!("previous_hash:    {:?}", self.previous_hash);
        for (idx, tx) in self.transactions.iter().enumerate() {
            let transaction = Transaction::deserialization(tx.to_vec());
            println!("the {}'th transaction is: {}", idx, transaction);
        }
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

#[derive(Debug, Clone)]

pub struct BlockChain {
    transactions_pool: Vec<Vec<u8>>,
    chain: Vec<Block>,
    blockchain_address: String,
}

impl BlockChain {
    const DIFFICULTY: usize = 3;
    const MINING_SENDER: &str = "THE BLOCKCHAIN";
    const MINING_REWARD: u64 = 1;

    pub fn new(address: String) -> Self {
        let mut bc = BlockChain {
            transactions_pool: Vec::<Vec<u8>>::new(),
            chain: Vec::<Block>::new(),
            blockchain_address: address,
        };

        let first_block = Block::new(0, vec![0 as u8; 32]);

        bc.chain.push(first_block);
        bc.mining();
        bc
    }

    pub fn create_block(&mut self, nonce: i32, previous_hash: Vec<u8>) {
        let mut block = Block::new(nonce, previous_hash);
        for tx in self.transactions_pool.iter() {
            block.transactions.push(tx.clone());
        }
        self.transactions_pool.clear();
        let now = Instant::now();
        let proof_hash = BlockChain::proof_of_work(&mut block);
        let elapsed = now.elapsed();
        println!(
            "computing time: {:?},proof hash for current block is: {:?}",
            elapsed, proof_hash
        );

        self.chain.push(block);
    }

    fn proof_of_work(block: &mut Block) -> String {
        loop {
            let hashed_block = block.hash();
            let hash_str = hex::encode(hashed_block);
            if hash_str[0..BlockChain::DIFFICULTY] == "0".repeat(BlockChain::DIFFICULTY) {
                return hash_str;
            }
            *block += 1;
        }
    }

    pub fn print_blockchain(&self) {
        for (idx, block) in self.chain.iter().enumerate() {
            println!("{} The Block {} {}", "=".repeat(25), idx, "=".repeat(25));
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

    pub fn add_transaction(&mut self, tx: &WalletTransaction) -> bool {
        if tx.sender == self.blockchain_address {
            print!("Miner can not send transaction to it self");
            return false;
        }

        if tx.sender != BlockChain::MINING_SENDER && !Wallet::verify_transaction(tx) {
            println!("Transaction is not valid!");
            return false;
        }

        if self.calculate_total_amount(tx.sender.clone()) < tx.amount as i64 {
            print!("Invalid balance");
            return false;
        }

        let btx = Transaction {
            sender_address: tx.sender.as_bytes().to_vec(),
            reciever_address: tx.recipeint.clone().into_bytes(),
            value: tx.amount,
        };

        for tx_in_pool in self.transactions_pool.iter() {
            if *tx_in_pool == btx.serialization() {
                return false;
            }
        }

        self.transactions_pool.push(btx.serialization());
        true
    }

    pub fn mining(&mut self) -> bool {
        /*
        if a block is mined, a transaction will created and the chain will send
        a coin to the miner
        */
        let tx = WalletTransaction {
            sender: BlockChain::MINING_SENDER.clone().into(),
            recipeint: self.blockchain_address.clone().into(),
            amount: BlockChain::MINING_REWARD,
            public_key: "".to_string(),
            signature: "".to_string(),
        };

        self.add_transaction(&tx);

        self.create_block(0, self.last_block().hash());

        true
    }

    pub fn calculate_total_amount(&self, address: String) -> i64 {
        let mut total_amount: i64 = 0;
        for i in 0..self.chain.len() {
            //we overload index operator before
            let block = &self[i];
            for t in block.transactions.iter() {
                let tx = Transaction::deserialization(t.clone());
                let value = tx.value;
                /*
                into is a trait used for converting one type into another type, String implement the trait
                of into with many different type, therefore we need to convert String to the trait with the
                right type,here we want string convert itself to Vec<u8>, then we need to convert String
                to type Into<Vec<u8>>.
                */
                if <String as Into<Vec<u8>>>::into(address.clone()) == tx.reciever_address {
                    total_amount += value as i64;
                }

                if <String as Into<Vec<u8>>>::into(address.clone()) == tx.sender_address {
                    total_amount -= value as i64;
                }
            }
        }

        total_amount
    }
}
