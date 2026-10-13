use hex;
use p256::PublicKey;
use p256::ecdsa::signature;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::PartialEq;
use std::collections::HashMap;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Block {
    nonce: i32,
    previous_hash: Vec<u8>,
    time_stamp: u128,
    transactions: Vec<Vec<u8>>,
}

impl Block {
    /// Deterministic genesis shared by all nodes. Fixed timestamp/nonce/empty
    /// txs so every node starts from the same hash — required for Nakamoto
    /// fork-finding (common ancestor always exists).
    pub fn genesis() -> Self {
        Block {
            nonce: 0,
            previous_hash: vec![0u8; 32],
            time_stamp: 0,
            transactions: Vec::new(),
        }
    }

    pub fn genesis_hash() -> Vec<u8> {
        Self::genesis().hash()
    }

    /// Reconstruct a block received over the network.
    pub fn from_parts(
        nonce: i32,
        previous_hash: Vec<u8>,
        time_stamp: u128,
        transactions: Vec<Vec<u8>>,
    ) -> Self {
        Block {
            nonce,
            previous_hash,
            time_stamp,
            transactions,
        }
    }

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

    pub fn nonce(&self) -> i32 {
        self.nonce
    }

    pub fn previous_hash(&self) -> &[u8] {
        &self.previous_hash
    }

    pub fn timestamp(&self) -> u128 {
        self.time_stamp
    }

    pub fn raw_transactions(&self) -> &[Vec<u8>] {
        &self.transactions
    }
}

#[derive(Debug, Clone)]

pub struct BlockChain {
    transactions_pool: Vec<Vec<u8>>,
    chain: Vec<Block>,
    blockchain_address: String,
}

impl BlockChain {
    pub(crate) const DIFFICULTY: usize = 3;
    pub(crate) const MINING_SENDER: &str = "THE BLOCKCHAIN";
    pub(crate) const MINING_REWARD: u64 = 1;
    /// Max clock skew accepted for incoming blocks (5 min in nanos).
    const MAX_FUTURE_SKEW_NANOS: u128 = 5 * 60 * 1_000_000_000;

    pub fn new(address: String) -> Self {
        let mut bc = BlockChain {
            transactions_pool: Vec::<Vec<u8>>::new(),
            chain: Vec::<Block>::new(),
            blockchain_address: address,
        };

        // Deterministic common genesis (height 0), then one mined block so a
        // fresh node still owns its miner reward. All nodes share block 0;
        // block 1+ may diverge and converge via longest-valid-chain rule.
        bc.chain.push(Block::genesis());
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

    pub fn add_transaction(&mut self, tx: &WalletTransaction) -> Result<(), String> {
        if tx.amount == 0 {
            return Err("amount must be greater than 0".to_string());
        }

        if tx.sender == tx.recipeint {
            return Err("sender and recipient must differ".to_string());
        }

        let is_reward = tx.sender == BlockChain::MINING_SENDER;
        if !is_reward && !Wallet::verify_transaction(tx) {
            println!("Transaction is not valid!");
            return Err("transaction signature verification failed".to_string());
        }

        // Minting transactions create funds out of thin air, so they are
        // exempt from the balance check. Everyone else must afford it from
        // confirmed (mined) balance minus what they already have queued in
        // the mempool (prevents double-spending unconfirmed funds).
        if !is_reward {
            let confirmed = self.calculate_total_amount(tx.sender.clone());
            let pending_out = self.pending_spent_by(&tx.sender);
            if confirmed - pending_out < tx.amount as i64 {
                println!(
                    "Invalid balance: {} has confirmed {} with {} pending",
                    tx.sender, confirmed, pending_out
                );
                return Err("insufficient confirmed balance (mine pending transactions first)".to_string());
            }
        }

        let btx = Transaction {
            sender_address: tx.sender.as_bytes().to_vec(),
            reciever_address: tx.recipeint.clone().into_bytes(),
            value: tx.amount,
        };

        // NOTE: no duplicate rejection here. The on-chain Transaction drops
        // the wallet signature, so two legitimate transfers with the same
        // sender/recipient/amount serialize identically. Rejecting "duplicates"
        // would block repeated sends from the webpage. Double-spends are
        // prevented by the balance check above instead.

        self.transactions_pool.push(btx.serialization());
        Ok(())
    }

    /// Total amount `address` has already queued as a sender in the mempool.
    /// Minting (MINING_SENDER) outputs are excluded since they create funds.
    fn pending_spent_by(&self, address: &str) -> i64 {
        let mut spent: i64 = 0;
        for raw in self.transactions_pool.iter() {
            let tx = Transaction::deserialization(raw.clone());
            if tx.sender_address == address.as_bytes().to_vec() {
                spent += tx.value as i64;
            }
        }
        spent
    }

    /// Mint `amount` new coins to `recipient` (test faucet). Uses the
    /// MINING_SENDER identity so it bypasses signature/balance checks.
    pub fn faucet(&mut self, recipient: String, amount: u64) -> Result<(), String> {
        if recipient.is_empty() {
            return Err("recipient address must not be empty".to_string());
        }
        if amount == 0 {
            return Err("amount must be greater than 0".to_string());
        }
        let tx = WalletTransaction {
            sender: BlockChain::MINING_SENDER.to_string(),
            recipeint: recipient,
            amount,
            public_key: String::new(),
            signature: String::new(),
        };
        self.add_transaction(&tx)
    }

    pub fn miner_address(&self) -> String {
        self.blockchain_address.clone()
    }

    pub fn pending_count(&self) -> usize {
        self.transactions_pool.len()
    }

    pub fn mining(&mut self) -> bool {
        /*
        if a block is mined, a transaction will created and the chain will send
        a coin to the miner
        */
        let tx = WalletTransaction {
            sender: BlockChain::MINING_SENDER.to_string(),
            recipeint: self.blockchain_address.clone(),
            amount: BlockChain::MINING_REWARD,
            public_key: String::new(),
            signature: String::new(),
        };

        if let Err(e) = self.add_transaction(&tx) {
            println!("mining reward failed: {e}");
        }

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

    pub fn get_chain(&self) -> &Vec<Block> {
        &self.chain
    }

    pub fn get_transactions(&self) -> Vec<Transaction> {
        let mut transactions = Vec::new();
        for block in self.chain.iter() {
            for t in block.transactions.iter() {
                transactions.push(Transaction::deserialization(t.clone()));
            }
        }
        transactions
    }

    // -- Nakamoto sync ----------------------------------------------------

    pub fn height(&self) -> usize {
        self.chain.len()
    }

    pub fn tip_hash(&self) -> Vec<u8> {
        self.last_block().hash()
    }

    pub fn hash_meets_difficulty(hash: &[u8]) -> bool {
        hex::encode(hash).starts_with(&"0".repeat(Self::DIFFICULTY))
    }

    fn now_nanos() -> u128 {
        SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    }

    /// Validate a single block against its predecessor.
    pub fn validate_block(block: &Block, prev: &Block) -> Result<(), String> {
        if block.previous_hash != prev.hash() {
            return Err("previous_hash does not match parent".to_string());
        }
        if block.time_stamp < prev.time_stamp {
            return Err("timestamp older than parent".to_string());
        }
        if block.time_stamp > Self::now_nanos() + Self::MAX_FUTURE_SKEW_NANOS {
            return Err("timestamp too far in the future".to_string());
        }
        let h = block.hash();
        if !Self::hash_meets_difficulty(&h) {
            return Err("proof-of-work does not meet difficulty".to_string());
        }
        for raw in block.transactions.iter() {
            if Transaction::try_deserialization(raw).is_none() {
                return Err("malformed transaction in block".to_string());
            }
        }
        Ok(())
    }

    /// Full chain validation: common genesis + linkage + PoW + ledger replay
    /// (no overspend; mints via `THE BLOCKCHAIN` create funds).
    pub fn validate_chain(chain: &[Block]) -> Result<(), String> {
        if chain.is_empty() {
            return Err("empty chain".to_string());
        }
        if chain[0].hash() != Block::genesis_hash() {
            return Err("genesis mismatch".to_string());
        }
        // Linkage / PoW / timestamps.
        for w in chain.windows(2) {
            Self::validate_block(&w[1], &w[0])?;
        }
        // Ledger replay.
        let mint_id = Self::MINING_SENDER.as_bytes().to_vec();
        let mut balances: HashMap<Vec<u8>, i64> = HashMap::new();
        for block in chain.iter() {
            for raw in block.transactions.iter() {
                let tx = Transaction::try_deserialization(raw)
                    .ok_or_else(|| "malformed transaction".to_string())?;
                if tx.value == 0 {
                    return Err("zero-value transaction".to_string());
                }
                if tx.sender_address == tx.reciever_address {
                    return Err("self-transfer".to_string());
                }
                if tx.sender_address == mint_id {
                    let e = balances.entry(tx.reciever_address.clone()).or_insert(0);
                    *e += tx.value as i64;
                } else {
                    let bal = balances.entry(tx.sender_address.clone()).or_insert(0);
                    if *bal < tx.value as i64 {
                        return Err("overspend in chain".to_string());
                    }
                    *bal -= tx.value as i64;
                    let e = balances.entry(tx.reciever_address.clone()).or_insert(0);
                    *e += tx.value as i64;
                }
            }
        }
        Ok(())
    }

    /// Longest-valid-chain rule. Returns true if we replaced.
    /// Keeps any mempool txs not yet confirmed, drops confirmed/invalid ones.
    pub fn replace_chain_if_longer_valid(&mut self, candidate: Vec<Block>) -> Result<bool, String> {
        if candidate.len() <= self.chain.len() {
            return Ok(false);
        }
        Self::validate_chain(&candidate)?;
        // Collect confirmed txs so we can drop them from the pool.
        let mut confirmed = std::collections::HashSet::new();
        for b in candidate.iter() {
            for raw in b.raw_transactions().iter() {
                confirmed.insert(raw.clone());
            }
        }
        self.chain = candidate;
        // Reconcile mempool: drop confirmed, drop malformed, drop now-overspending.
        // Drain first to avoid borrowing self while computing balances.
        let pool: Vec<Vec<u8>> = std::mem::take(&mut self.transactions_pool);
        let mut kept: Vec<Vec<u8>> = Vec::new();
        for raw in pool {
            if confirmed.contains(&raw) {
                continue;
            }
            let Some(tx) = Transaction::try_deserialization(&raw) else {
                continue;
            };
            let mint_id = Self::MINING_SENDER.as_bytes().to_vec();
            if tx.sender_address == mint_id {
                kept.push(raw);
                continue;
            }
            let sender = String::from_utf8_lossy(&tx.sender_address).into_owned();
            let confirmed_bal = self.calculate_total_amount(sender.clone());
            let pending_out: i64 = kept
                .iter()
                .filter_map(|r| Transaction::try_deserialization(r))
                .filter(|t| t.sender_address == tx.sender_address)
                .map(|t| t.value as i64)
                .sum();
            if confirmed_bal - pending_out >= tx.value as i64 {
                kept.push(raw);
            }
        }
        self.transactions_pool = kept;
        Ok(true)
    }

    /// Append a single pushed block if it directly extends our tip.
    pub fn append_block_if_extends(&mut self, block: Block) -> Result<bool, String> {
        let tip = self.last_block().clone();
        Self::validate_block(&block, &tip)?;
        // Drop newly confirmed pool entries.
        let confirmed: std::collections::HashSet<Vec<u8>> =
            block.raw_transactions().iter().cloned().collect();
        self.transactions_pool.retain(|r| !confirmed.contains(r));
        self.chain.push(block);
        Ok(true)
    }

    pub fn pending_raw(&self) -> Vec<Vec<u8>> {
        self.transactions_pool.clone()
    }

    /// Merge mempool entries learned from a peer. Entries are raw on-chain
    /// txs (signatures already checked at the entry node); we check structure
    /// + spendability against our current chain. Returns newly added count.
    pub fn merge_pending_raw(&mut self, raws: Vec<Vec<u8>>) -> usize {
        let mut added = 0;
        for raw in raws {
            if self.transactions_pool.contains(&raw) {
                continue;
            }
            let Some(tx) = Transaction::try_deserialization(&raw) else {
                continue;
            };
            if tx.value == 0 || tx.sender_address == tx.reciever_address {
                continue;
            }
            let mint_id = Self::MINING_SENDER.as_bytes().to_vec();
            if tx.sender_address != mint_id {
                let sender = String::from_utf8_lossy(&tx.sender_address).into_owned();
                let confirmed_bal = self.calculate_total_amount(sender.clone());
                let pending_out = self.pending_spent_by(&sender);
                if confirmed_bal - pending_out < tx.value as i64 {
                    continue;
                }
            }
            self.transactions_pool.push(raw);
            added += 1;
        }
        added
    }

    pub fn block_by_hash(&self, hash: &[u8]) -> Option<(usize, &Block)> {
        self.chain
            .iter()
            .enumerate()
            .find(|(_, b)| b.hash() == hash)
    }
}
