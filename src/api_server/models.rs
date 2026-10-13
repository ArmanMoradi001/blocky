//! API request / response DTOs.
//!
//! Keeps `api.rs` focused on routing + handlers. No business logic lives here.

use serde::{Deserialize, Serialize};

use crate::blockchain::transaction::Transaction as BlockchainTransaction;

// GET /amount/{address} -> { "amount": 123 }
#[derive(Serialize)]
pub struct QueryAmount {
    pub amount: i64,
}

#[derive(Deserialize, Debug)]
pub struct TransactionRequest {
    pub private_key: String,
    pub public_key: String,
    pub blockchain_address: String,
    pub recipient_address: String,
    pub amount: String,
}

// Keep old name as alias so existing clients keep working.
pub type Transaction = TransactionRequest;

#[derive(Deserialize, Debug)]
pub struct FaucetRequest {
    pub recipient_address: String,
    pub amount: String,
}

#[derive(Serialize, Debug)]
pub struct TransferResponse {
    pub status: String,
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
    pub pending_in_pool: usize,
    pub note: String,
}

/// Human-readable transaction for API responses. The chain stores addresses
/// as raw bytes; the webpage needs plain strings.
#[derive(Serialize, Debug)]
pub struct TransactionView {
    pub sender: String,
    pub recipient: String,
    pub amount: u64,
}

impl From<&BlockchainTransaction> for TransactionView {
    fn from(tx: &BlockchainTransaction) -> Self {
        TransactionView {
            sender: String::from_utf8_lossy(&tx.sender_address).into_owned(),
            recipient: String::from_utf8_lossy(&tx.reciever_address).into_owned(),
            amount: tx.value,
        }
    }
}

/// Human-readable block for API responses (hashes as hex, txs decoded).
#[derive(Serialize, Debug)]
pub struct BlockView {
    pub index: usize,
    pub nonce: i32,
    pub previous_hash: String,
    pub hash: String,
    pub timestamp: u128,
    pub transactions: Vec<TransactionView>,
}

// -- Nakamoto sync DTOs ---------------------------------------------------
// Wire format uses hex strings so any HTTP client can reconstruct blocks.

/// `GET /chain/height` — cheap comparison before a full pull.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChainHeightResponse {
    pub height: usize,
    pub tip_hash: String,
    pub genesis_hash: String,
}

/// One full block on the wire.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockDto {
    pub nonce: i32,
    pub previous_hash: String,
    pub timestamp: u128,
    pub transactions: Vec<String>,
    pub hash: String,
}

/// `GET /chain` — full chain for longest-valid-chain sync (small chains only).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChainResponse {
    pub height: usize,
    pub tip_hash: String,
    pub genesis_hash: String,
    pub blocks: Vec<BlockDto>,
}

/// Header for `GET /chain/headers` (header-first sync without bodies).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct BlockHeaderDto {
    pub index: usize,
    pub hash: String,
    pub previous_hash: String,
    pub nonce: i32,
    pub timestamp: u128,
    pub tx_count: usize,
}

/// `GET /chain/pending` / `POST /chain/pending` — mempool reconciliation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PendingResponse {
    pub pending: Vec<String>,
}
