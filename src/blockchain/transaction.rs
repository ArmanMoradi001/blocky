use super::blockchain::Serialization;
use core::fmt;
use std::fmt::{Display, Formatter};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct Transaction {
    pub sender_address: Vec<u8>,
    pub reciever_address: Vec<u8>,
    pub value: u64,
}

impl Transaction {
    pub fn new(sender: Vec<u8>, reciever: Vec<u8>, value: u64) -> Self {
        Transaction {
            sender_address: sender,
            reciever_address: reciever,
            value,
        }
    }

    /// Non-panicking decode used by p2p validation. Returns `None` on
    /// truncated/trailing bytes instead of asserting.
    pub fn try_deserialization(bytes: &[u8]) -> Option<Transaction> {
        let mut pos = 0usize;
        if bytes.len() < pos + 8 {
            return None;
        }
        let sender_len = u64::from_be_bytes(bytes[pos..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        // Sanity cap: addresses are Base58 strings (< 100 bytes).
        if sender_len > 1024 || bytes.len() < pos + sender_len {
            return None;
        }
        let sender_address = bytes[pos..pos + sender_len].to_vec();
        pos += sender_len;

        if bytes.len() < pos + 8 {
            return None;
        }
        let receiver_len = u64::from_be_bytes(bytes[pos..pos + 8].try_into().ok()?) as usize;
        pos += 8;
        if receiver_len > 1024 || bytes.len() < pos + receiver_len {
            return None;
        }
        let reciever_address = bytes[pos..pos + receiver_len].to_vec();
        pos += receiver_len;

        if bytes.len() < pos + 8 {
            return None;
        }
        let value = u64::from_be_bytes(bytes[pos..pos + 8].try_into().ok()?);
        pos += 8;

        if pos != bytes.len() {
            return None;
        }

        Some(Transaction {
            sender_address,
            reciever_address,
            value,
        })
    }
}

impl Serialization<Transaction> for Transaction {
    fn serialization(&self) -> Vec<u8> {
        let mut bin = Vec::new();

        let sender_len = self.sender_address.len() as u64;
        bin.extend(sender_len.to_be_bytes());
        bin.extend(&self.sender_address);

        let receiver_len = self.reciever_address.len() as u64;
        bin.extend(receiver_len.to_be_bytes());
        bin.extend(&self.reciever_address);

        bin.extend(self.value.to_be_bytes());

        bin
    }

    fn deserialization(bytes: Vec<u8>) -> Transaction {
        match Self::try_deserialization(&bytes) {
            Some(tx) => tx,
            None => panic!(
                "Transaction::deserialization: malformed {} byte(s)",
                bytes.len()
            ),
        }
    }
}

impl fmt::Display for Transaction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}\n Sender Address: {:?}\n Reciever Address: {:?}\n Value: {:?}\n {} ",
            "-".repeat(40),
            self.sender_address,
            self.reciever_address,
            self.value,
            "-".repeat(40)
        )
    }
}
