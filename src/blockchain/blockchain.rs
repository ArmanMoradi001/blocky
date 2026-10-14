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
            // The signature + key travel with the transaction from here on:
            // every validation path re-verifies them, so a peer cannot forge
            // spends from an address it does not own (mints carry neither).
            public_key: hex::decode(tx.public_key.trim())
                .map_err(|_| "bad public_key hex".to_string())?,
            signature: hex::decode(tx.signature.trim())
                .map_err(|_| "bad signature hex".to_string())?,
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

    /// Verify an on-chain transaction's signature. Mints from
    /// `MINING_SENDER` create funds and carry no signature; everything else
    /// must present a valid 64-byte key + 64-byte signature over the exact
    /// spend. This is what stops a peer forging spends from an address it
    /// does not own — call it on every path that accepts blocks or mempool
    /// entries.
    fn verify_onchain_tx(tx: &Transaction) -> Result<(), String> {
        if tx.sender_address == Self::MINING_SENDER.as_bytes().to_vec() {
            return Ok(());
        }
        if tx.public_key.len() != 64 || tx.signature.len() != 64 {
            return Err("missing transaction signature".to_string());
        }
        let sender = String::from_utf8(tx.sender_address.clone())
            .map_err(|_| "sender address is not valid UTF-8".to_string())?;
        let recipeint = String::from_utf8(tx.reciever_address.clone())
            .map_err(|_| "recipient address is not valid UTF-8".to_string())?;
        let wt = WalletTransaction {
            sender,
            recipeint,
            amount: tx.value,
            public_key: hex::encode(&tx.public_key),
            signature: hex::encode(&tx.signature),
        };
        if !Wallet::verify_transaction(&wt) {
            return Err("transaction signature verification failed".to_string());
        }
        Ok(())
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
        for raw in block.transactions.iter() {
            // Safe to unwrap: checked well-formed just above.
            let tx = Transaction::try_deserialization(raw)
                .ok_or_else(|| "malformed transaction in block".to_string())?;
            Self::verify_onchain_tx(&tx)?;
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
                // Re-verify the persisted signature: blocks learned from
                // peers must prove every spend, not just a valid hash.
                Self::verify_onchain_tx(&tx)?;
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
    /// txs; we check structure, signatures, and spendability against our
    /// current chain — a peer is never trusted on its word. Entries already
    /// confirmed in our local chain are skipped — without this, the
    /// background pull re-imports txs from lagging peers that haven't adopted
    /// our latest block yet, and they get mined a second time (double
    /// mint/spend). Returns newly added count.
    pub fn merge_pending_raw(&mut self, raws: Vec<Vec<u8>>) -> usize {
        let confirmed: std::collections::HashSet<Vec<u8>> = self
            .chain
            .iter()
            .flat_map(|b| b.raw_transactions().iter().cloned())
            .collect();
        let mut added = 0;
        for raw in raws {
            if confirmed.contains(&raw) || self.transactions_pool.contains(&raw) {
                continue;
            }
            let Some(tx) = Transaction::try_deserialization(&raw) else {
                continue;
            };
            if tx.value == 0 || tx.sender_address == tx.reciever_address {
                continue;
            }
            // Forged spends die here even if the economics would allow them.
            if Self::verify_onchain_tx(&tx).is_err() {
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

// -- tests -----------------------------------------------------------------
// T1.1 chain validation, T1.2 mempool merge, T1.4 sync fork-choice.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::wallet::wallet::Wallet;

    fn miner_chain() -> BlockChain {
        BlockChain::new("miner-addr".to_string())
    }

    fn raw_tx(sender: &str, recipient: &str, value: u64) -> Vec<u8> {
        Transaction::new(
            sender.as_bytes().to_vec(),
            recipient.as_bytes().to_vec(),
            value,
        )
        .serialization()
    }

    /// On-chain raw form of a transfer actually signed by `w`. The signature
    /// is valid — tests use these to prove that bad *economics* (overspend,
    /// double-spend, zero value, self-transfer) are rejected even when the
    /// cryptography checks out.
    fn signed_raw(w: &Wallet, recipient: &str, amount: u64) -> Vec<u8> {
        let wt = w.sign_transaction(&recipient.to_string(), amount);
        Transaction::signed(
            wt.sender.into_bytes(),
            wt.recipeint.into_bytes(),
            wt.amount,
            hex::decode(wt.public_key).unwrap(),
            hex::decode(wt.signature).unwrap(),
        )
        .serialization()
    }

    /// Stuff raw txs straight into the pool (bypasses entry checks) and mine
    /// them into a block. Lets tests craft overspending / zero-value /
    /// self-transfer chains that `add_transaction` would refuse.
    fn mine_with(bc: &mut BlockChain, txs: Vec<Vec<u8>>) {
        bc.transactions_pool = txs;
        let tip = bc.last_block().hash();
        bc.create_block(0, tip);
    }

    /// A block with correct linkage/timestamps but no proof-of-work.
    fn unmined_block(prev: &Block) -> Block {
        let mut nonce = 0i32;
        loop {
            let b = Block::from_parts(nonce, prev.hash(), prev.timestamp(), vec![]);
            if !BlockChain::hash_meets_difficulty(&b.hash()) {
                return b;
            }
            nonce += 1;
        }
    }

    /// Fresh chain where `addr` owns exactly 100 confirmed coins.
    fn funded(addr: &str) -> BlockChain {
        let mut bc = miner_chain();
        bc.faucet(addr.to_string(), 100).unwrap();
        bc.mining();
        bc
    }

    /// Fresh chain where wallet `w` owns exactly 100 confirmed coins
    /// (so tests can sign spends from it).
    fn fund_wallet(w: &Wallet) -> BlockChain {
        let mut bc = miner_chain();
        bc.faucet(w.address_str(), 100).unwrap();
        bc.mining();
        bc
    }

    // -- T1.1: chain validation ------------------------------------------

    #[test]
    fn fresh_chain_is_valid() {
        let bc = miner_chain();
        assert_eq!(bc.height(), 2); // genesis + first miner reward
        assert!(BlockChain::validate_chain(bc.get_chain()).is_ok());
        assert_eq!(bc.calculate_total_amount("miner-addr".to_string()), 1);
    }

    #[test]
    fn faucet_then_mine_confirms_and_validates() {
        let bc = funded("alice");
        assert_eq!(bc.calculate_total_amount("alice".to_string()), 100);
        assert!(BlockChain::validate_chain(bc.get_chain()).is_ok());
    }

    #[test]
    fn mempool_entry_rejects_forged_signature() {
        let w = Wallet::new();
        let mut bc = miner_chain();
        bc.faucet(w.address_str(), 50).unwrap();
        bc.mining();
        // Sign, then tamper: the signature no longer matches the payload.
        let mut wt = w.sign_transaction(&"bob".to_string(), 10);
        wt.amount = 1000;
        let err = bc.add_transaction(&wt).unwrap_err();
        assert!(err.contains("signature"), "unexpected: {err}");
    }

    #[test]
    fn mempool_entry_rejects_overspend() {
        let w = Wallet::new();
        let mut bc = miner_chain();
        bc.faucet(w.address_str(), 10).unwrap();
        bc.mining();
        let wt = w.sign_transaction(&"bob".to_string(), 11);
        let err = bc.add_transaction(&wt).unwrap_err();
        assert!(err.contains("insufficient"), "unexpected: {err}");
    }

    #[test]
    fn chain_rejects_overspend() {
        let w = Wallet::new();
        let mut bc = fund_wallet(&w); // owns 100
        mine_with(&mut bc, vec![signed_raw(&w, "bob", 1000)]);
        let err = BlockChain::validate_chain(bc.get_chain()).unwrap_err();
        assert!(err.contains("overspend"), "unexpected: {err}");
    }

    #[test]
    fn chain_rejects_double_spend() {
        let w = Wallet::new();
        let mut bc = fund_wallet(&w); // owns 100
        mine_with(
            &mut bc,
            vec![signed_raw(&w, "bob", 60), signed_raw(&w, "carol", 60)],
        );
        let err = BlockChain::validate_chain(bc.get_chain()).unwrap_err();
        assert!(err.contains("overspend"), "unexpected: {err}");
    }

    #[test]
    fn chain_rejects_zero_value() {
        let w = Wallet::new();
        let mut bc = fund_wallet(&w);
        mine_with(&mut bc, vec![signed_raw(&w, "bob", 0)]);
        let err = BlockChain::validate_chain(bc.get_chain()).unwrap_err();
        assert!(err.contains("zero-value"), "unexpected: {err}");
    }

    #[test]
    fn chain_rejects_self_transfer() {
        let w = Wallet::new();
        let mut bc = fund_wallet(&w);
        let addr = w.address_str();
        mine_with(&mut bc, vec![signed_raw(&w, &addr, 5)]);
        let err = BlockChain::validate_chain(bc.get_chain()).unwrap_err();
        assert!(err.contains("self-transfer"), "unexpected: {err}");
    }

    #[test]
    fn chain_rejects_forged_signature() {
        // Well-formed but unsignable: random 64-byte key/signature that no
        // wallet ever produced. Pre-S2 this validated; now it must not.
        let mut bc = funded("alice");
        let forged = Transaction::signed(
            b"alice".to_vec(),
            b"bob".to_vec(),
            10,
            vec![7u8; 64],
            vec![9u8; 64],
        )
        .serialization();
        mine_with(&mut bc, vec![forged]);
        let err = BlockChain::validate_chain(bc.get_chain()).unwrap_err();
        assert!(err.contains("signature"), "unexpected: {err}");
    }

    #[test]
    fn block_rejects_bad_proof_of_work() {
        let bc = miner_chain();
        let tip = bc.last_block().clone();
        let bad = unmined_block(&tip);
        let err = BlockChain::validate_block(&bad, &tip).unwrap_err();
        assert!(err.contains("proof-of-work"), "unexpected: {err}");
    }

    #[test]
    fn block_rejects_bad_previous_hash() {
        let bc = miner_chain();
        let tip = bc.last_block().clone();
        let bad = Block::from_parts(0, vec![9u8; 32], tip.timestamp(), vec![]);
        let err = BlockChain::validate_block(&bad, &tip).unwrap_err();
        assert!(err.contains("previous_hash"), "unexpected: {err}");
    }

    #[test]
    fn block_rejects_future_timestamp() {
        let bc = miner_chain();
        let tip = bc.last_block().clone();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let bad = Block::from_parts(0, tip.hash(), now + 3_600_000_000_000, vec![]);
        let err = BlockChain::validate_block(&bad, &tip).unwrap_err();
        assert!(err.contains("future"), "unexpected: {err}");
    }

    #[test]
    fn block_rejects_older_timestamp() {
        let bc = funded("alice");
        let tip = bc.last_block().clone();
        let bad = Block::from_parts(
            0,
            tip.hash(),
            tip.timestamp().saturating_sub(1),
            vec![],
        );
        // Either the timestamp check fires, or (if tip ts is 0) PoW fails —
        // both are rejections of a backdated block.
        assert!(BlockChain::validate_block(&bad, &tip).is_err());
    }

    #[test]
    fn chain_rejects_genesis_mismatch() {
        let err =
            BlockChain::validate_chain(&[Block::new(0, vec![0u8; 32])]).unwrap_err();
        assert!(err.contains("genesis"), "unexpected: {err}");
    }

    #[test]
    fn chain_rejects_empty() {
        let err = BlockChain::validate_chain(&[]).unwrap_err();
        assert!(err.contains("empty"), "unexpected: {err}");
    }

    // Desired behavior once the S4 overflow fix lands (`u64` checked
    // arithmetic in balance checks): a `u64::MAX` spend must be rejected even
    // though it is correctly signed. Today it wraps to a negative `i64` and
    // bypasses the check, so this stays ignored until the fix.
    #[test]
    #[ignore = "S4: u64::MAX wraps to negative i64 and bypasses the balance check"]
    fn mempool_entry_rejects_huge_amount() {
        let w = Wallet::new();
        let mut bc = miner_chain();
        bc.faucet(w.address_str(), 10).unwrap();
        bc.mining();
        let wt = w.sign_transaction(&"bob".to_string(), u64::MAX);
        assert!(bc.add_transaction(&wt).is_err());
    }

    // -- T1.2: mempool merge ---------------------------------------------

    #[test]
    fn merge_skips_already_confirmed() {
        let mut bc = funded("alice");
        assert_eq!(bc.pending_count(), 0);
        let confirmed = bc.get_chain()[2].raw_transactions()[0].clone();
        assert_eq!(bc.merge_pending_raw(vec![confirmed]), 0);
        assert_eq!(bc.pending_count(), 0);
    }

    #[test]
    fn merge_skips_pool_duplicates() {
        let mut bc = miner_chain();
        let raw = raw_tx(BlockChain::MINING_SENDER, "alice", 25);
        assert_eq!(bc.merge_pending_raw(vec![raw.clone()]), 1);
        assert_eq!(bc.merge_pending_raw(vec![raw]), 0);
        assert_eq!(bc.pending_count(), 1);
    }

    #[test]
    fn merge_skips_malformed() {
        let mut bc = miner_chain();
        let garbage = vec![vec![0u8; 3], b"not-a-tx".to_vec(), vec![255u8; 8]];
        assert_eq!(bc.merge_pending_raw(garbage), 0);
        assert_eq!(bc.pending_count(), 0);
    }

    #[test]
    fn merge_skips_zero_value_and_self_transfer() {
        let w = Wallet::new();
        let mut bc = fund_wallet(&w);
        let addr = w.address_str();
        assert_eq!(
            bc.merge_pending_raw(vec![signed_raw(&w, "b", 0), signed_raw(&w, &addr, 5)]),
            0
        );
        assert_eq!(bc.pending_count(), 0);
    }

    #[test]
    fn merge_skips_overspend_but_accepts_affordable() {
        let w = Wallet::new();
        let mut bc = fund_wallet(&w); // owns 100
        assert_eq!(bc.merge_pending_raw(vec![signed_raw(&w, "bob", 101)]), 0);
        assert_eq!(bc.merge_pending_raw(vec![signed_raw(&w, "bob", 100)]), 1);
        assert_eq!(bc.pending_count(), 1);
    }

    #[test]
    fn merge_accepts_spendable_peer_tx() {
        // A wallet-signed spend, funded on both chains (as gossip would see).
        let w = Wallet::new();
        let mut a = miner_chain();
        a.faucet(w.address_str(), 50).unwrap();
        a.mining();

        let mut b = miner_chain();
        b.faucet(w.address_str(), 50).unwrap();
        b.mining();
        assert_eq!(b.merge_pending_raw(vec![signed_raw(&w, "recipient-x", 20)]), 1);
    }

    #[test]
    fn merge_rejects_forged_peer_tx() {
        // Affordable economics, garbage signature: gossip must not launder it.
        let mut bc = funded("alice"); // alice owns 100
        let forged = Transaction::signed(
            b"alice".to_vec(),
            b"bob".to_vec(),
            10,
            vec![7u8; 64],
            vec![9u8; 64],
        )
        .serialization();
        assert_eq!(bc.merge_pending_raw(vec![forged]), 0);
        assert_eq!(bc.pending_count(), 0);
    }

    #[test]
    fn replace_drops_confirmed_pool_but_keeps_unconfirmed() {
        let w = Wallet::new();
        let mut a = fund_wallet(&w); // h3, owns 100
        let spend = signed_raw(&w, "bob", 30);
        a.transactions_pool.push(spend.clone());
        assert_eq!(a.pending_count(), 1);

        // Peer chain confirming the spend: adopt, pool entry must go.
        let mut confirming = BlockChain::new("miner-addr".to_string());
        confirming.chain = a.get_chain().clone();
        confirming.transactions_pool = vec![spend.clone()];
        confirming.create_block(0, confirming.last_block().hash());
        assert!(a
            .replace_chain_if_longer_valid(confirming.get_chain().clone())
            .unwrap());
        assert_eq!(a.height(), 4);
        assert_eq!(a.pending_count(), 0, "confirmed tx must leave the pool");
        assert!(BlockChain::validate_chain(a.get_chain()).is_ok());

        // Peer chain not containing the spend: adopt, pool entry must stay.
        let mut a = fund_wallet(&w);
        a.transactions_pool.push(spend);
        let mut longer = BlockChain::new("miner-addr".to_string());
        longer.chain = a.get_chain().clone();
        longer.create_block(0, longer.last_block().hash()); // empty block
        assert!(a
            .replace_chain_if_longer_valid(longer.get_chain().clone())
            .unwrap());
        assert_eq!(a.pending_count(), 1, "unconfirmed tx must survive reorg");
    }

    // -- T1.4: sync fork-choice ------------------------------------------

    #[test]
    fn fork_choice_adopts_taller_valid_chain() {
        let mut a = miner_chain();
        let mut b = miner_chain();
        b.mining();
        b.mining(); // height 4
        let tip = b.tip_hash();
        assert!(a
            .replace_chain_if_longer_valid(b.get_chain().clone())
            .unwrap());
        assert_eq!(a.height(), 4);
        assert_eq!(a.tip_hash(), tip);
    }

    #[test]
    fn fork_choice_rejects_taller_invalid_chain() {
        let w = Wallet::new();
        let mut b = fund_wallet(&w);
        mine_with(&mut b, vec![signed_raw(&w, "bob", 10_000)]); // overspends
        let mut a = miner_chain();
        let (height, tip) = (a.height(), a.tip_hash());
        let err = a
            .replace_chain_if_longer_valid(b.get_chain().clone())
            .unwrap_err();
        assert!(err.contains("overspend"), "unexpected: {err}");
        assert_eq!(a.height(), height, "local chain must be untouched");
        assert_eq!(a.tip_hash(), tip, "local tip must be untouched");
    }

    #[test]
    fn fork_choice_keeps_local_on_equal_height() {
        let mut a = BlockChain::new("miner-a".to_string());
        let b = BlockChain::new("miner-b".to_string());
        assert_eq!(a.height(), b.height());
        let tip = a.tip_hash();
        assert_eq!(
            a.replace_chain_if_longer_valid(b.get_chain().clone()),
            Ok(false)
        );
        assert_eq!(a.tip_hash(), tip);
    }

    #[test]
    fn fork_choice_rejects_shorter_chain() {
        let mut b = miner_chain();
        b.mining(); // height 3
        let mut a = BlockChain::new("miner-a".to_string());
        a.chain = b.get_chain().clone();
        a.mining(); // height 4
        assert_eq!(
            a.replace_chain_if_longer_valid(b.get_chain().clone()),
            Ok(false)
        );
        assert_eq!(a.height(), 4);
    }

    #[test]
    fn fork_choice_rejects_foreign_genesis() {
        let mut b = miner_chain();
        b.mining();
        b.mining(); // height 4
        let mut candidate = b.get_chain().clone();
        candidate[0] = Block::from_parts(99, vec![0u8; 32], 0, vec![]);
        let mut a = miner_chain();
        let err = a.replace_chain_if_longer_valid(candidate).unwrap_err();
        assert!(err.contains("genesis"), "unexpected: {err}");
    }
}
