use rand::rngs::SysRng;
use rand_core::UnwrapErr;
use ripemd160::{Digest as ripDigest, Ripemd160};
use serde::Serialize;
use sha2::{Digest, Sha256};

use p256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};

use hex;

pub struct Wallet {
    signing_key: SigningKey,
    verifying_key: VerifyingKey,
    address: String,
}

#[derive(Serialize, Clone, Debug)]

pub struct WalletData {
    pub public_key: String,
    pub private_key: String,
    pub blockchain_address: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Transaction {
    pub sender: String,
    pub recipeint: String,
    pub amount: u64,
    pub public_key: String,
    pub signature: String,
}

impl Wallet {
    pub fn new() -> Self {
        let mut rng = UnwrapErr(SysRng);
        let sk = SigningKey::random(&mut rng);
        let vk = sk.verifying_key().clone();
        let address = Self::generate_address(&vk);

        Wallet {
            signing_key: sk,
            verifying_key: vk,

            address,
        }
    }

    fn generate_address(vk: &VerifyingKey) -> String {
        let key_points = vk.to_sec1_point(false);
        let (Some(x), Some(y)) = (key_points.x(), key_points.y()) else {
            panic!("address generation went wrong")
        };
        let mut key_points_bytes = Vec::with_capacity(x.len() + y.len());
        key_points_bytes.extend_from_slice(x);
        key_points_bytes.extend_from_slice(y);

        let hash = Sha256::digest(&key_points_bytes);

        let mut hasher = Ripemd160::new();
        hasher.update(&hash);
        let mut versioned_payload = hasher.finalize().to_vec();

        versioned_payload.insert(0, 0x00);

        let hash2 = Sha256::digest(&versioned_payload);
        let hash3 = Sha256::digest(hash2);

        let checksum = &hash3[0..4];
        let full_hash = [versioned_payload, checksum.to_vec()].concat();

        bs58::encode(full_hash).into_string()
    }

    pub fn private_key_str(&self) -> String {
        hex::encode(self.signing_key.to_bytes())
    }

    pub fn public_key_str(&self) -> String {
        let key_point = self.verifying_key.to_sec1_point(false);
        let point_x = key_point.x().unwrap();
        let point_y = key_point.y().unwrap();

        let pub_str = hex::encode(point_x) + hex::encode(point_y).as_str();

        pub_str
        // (hex::encode(point_x), hex::encode(point_y))
    }

    pub fn address_str(&self) -> String {
        /*
        1. do sha256 on public key x, y
        2. do rimp on the step one result and we will get 20 byte
        3. add a version byte on the head of strp 2 result
        4. do sha256 on result of step3
        5. do sha256 on result of step 4
        6. take the 4 first byte of the step 5 as checksum
        7. append the 4 bytes from step 6 to the end of the result from step3
        8. encode the result of step 7 by base58
         */
        self.address.clone()
    }

    pub fn sign_transaction(&self, reciever: &String, amount: u64) -> Transaction {
        let mut transaction = Transaction {
            sender: self.address.clone(),
            recipeint: reciever.clone(),
            amount: amount,
            public_key: self.public_key_str(),
            signature: String::new(),
        };

        let serialized_str = serde_json::to_string(&transaction).unwrap();
        let serialized = serialized_str.as_bytes();
        let sign: Signature = self.signing_key.sign(serialized);

        transaction.signature = hex::encode(sign.to_bytes());
        transaction
    }

    pub fn verify_transaction(transaction: &Transaction) -> bool {
        // 1. Decode the hex signature; any malformed input fails verification.
        let sig_bytes = match hex::decode(&transaction.signature) {
            Ok(b) => b,
            Err(_) => return false,
        };
        let signature = match Signature::from_slice(&sig_bytes) {
            Ok(s) => s,
            Err(_) => return false,
        };

        // 2. Rebuild the verifying key.
        // `public_key_str()` encodes raw x || y as hex (32 + 32 bytes).
        let pubkey_bytes = match hex::decode(&transaction.public_key) {
            Ok(b) => b,
            Err(_) => return false,
        };
        if pubkey_bytes.len() != 64 {
            return false;
        }
        let mut sec1_bytes = Vec::with_capacity(65);
        sec1_bytes.push(0x04); // uncompressed point prefix
        sec1_bytes.extend_from_slice(&pubkey_bytes);
        let verifying_key = match VerifyingKey::from_sec1_bytes(&sec1_bytes) {
            Ok(k) => k,
            Err(_) => return false,
        };

        // 3. Bind the key to the claimed sender address so an attacker can't
        // swap in their own public key for someone else's address.
        if Self::generate_address(&verifying_key) != transaction.sender {
            return false;
        }

        // 4. Reconstruct the exact payload that was signed: the transaction
        // serialized with an empty signature (see `sign_transaction`).
        let mut unsigned = transaction.clone();
        unsigned.signature = String::new();
        let message = match serde_json::to_string(&unsigned) {
            Ok(s) => s,
            Err(_) => return false,
        };

        verifying_key.verify(message.as_bytes(), &signature).is_ok()
    }

    pub fn try_new_from(
        public_key_hex: &str,
        private_key_hex: &str,
        address: &str,
    ) -> Result<Self, String> {
        let sk_bytes =
            hex::decode(private_key_hex).map_err(|_| "invalid private_key hex".to_string())?;
        let signing_key = SigningKey::from_slice(&sk_bytes)
            .map_err(|_| "invalid private_key (not a valid secp256r1 scalar)".to_string())?;
        let verifying_key = signing_key.verifying_key().clone();

        // Consistency check: derived public key must match the supplied one.
        let derived_pub = {
            let key_point = verifying_key.to_sec1_point(false);
            let (Some(point_x), Some(point_y)) = (key_point.x(), key_point.y()) else {
                return Err("failed to derive public key".to_string());
            };
            hex::encode(point_x) + hex::encode(point_y).as_str()
        };
        if derived_pub != public_key_hex {
            return Err("public_key does not match private_key".to_string());
        }
        let derived_addr = Self::generate_address(&verifying_key);
        if derived_addr != address {
            return Err("blockchain_address does not match keys".to_string());
        }

        Ok(Wallet {
            signing_key,
            verifying_key,
            address: address.to_string(),
        })
    }

    pub fn new_from(public_key_hex: &str, private_key_hex: &str, address: &str) -> Self {
        Self::try_new_from(public_key_hex, private_key_hex, address)
            .expect("invalid wallet keys")
    }

    pub fn get_wallet_data(&self) -> WalletData {
        WalletData {
            public_key: self.public_key_str(),
            private_key: self.private_key_str(),
            blockchain_address: self.address_str(),
        }
    }

    /// Address for a raw uncompressed public key (64-byte x || y hex).
    /// `pub(crate)` for the JS-compat cross test below.
    pub(crate) fn address_for_pubkey_hex(public_key_hex: &str) -> Option<String> {
        let bytes = hex::decode(public_key_hex).ok()?;
        if bytes.len() != 64 {
            return None;
        }
        let mut sec1 = Vec::with_capacity(65);
        sec1.push(0x04);
        sec1.extend_from_slice(&bytes);
        let vk = VerifyingKey::from_sec1_bytes(&sec1).ok()?;
        Some(Self::generate_address(&vk))
    }
}

// -- tests -----------------------------------------------------------------
// T1.5: wallet signing / verification + key reconstruction.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_verify_round_trip() {
        let w = Wallet::new();
        for amount in [1u64, 20, 1_000_000] {
            let tx = w.sign_transaction(&"recipient-addr".to_string(), amount);
            assert!(Wallet::verify_transaction(&tx));
        }
    }

    #[test]
    fn verify_rejects_tampered_fields() {
        let w = Wallet::new();
        let base = w.sign_transaction(&"recipient-addr".to_string(), 25);

        let mut t = base.clone();
        t.amount = 26;
        assert!(!Wallet::verify_transaction(&t), "tampered amount must fail");

        let mut t = base.clone();
        t.recipeint = "someone-else".to_string();
        assert!(
            !Wallet::verify_transaction(&t),
            "tampered recipient must fail"
        );

        let mut t = base.clone();
        t.sender = Wallet::new().address_str();
        assert!(!Wallet::verify_transaction(&t), "swapped sender must fail");
    }

    #[test]
    fn verify_rejects_swapped_public_key() {
        // Attacker replaces the victim's key with their own: the derived
        // address no longer matches the claimed sender.
        let victim = Wallet::new();
        let attacker = Wallet::new();
        let mut tx = victim.sign_transaction(&"recipient-addr".to_string(), 5);
        tx.public_key = attacker.public_key_str();
        assert!(!Wallet::verify_transaction(&tx));
    }

    #[test]
    fn verify_rejects_malformed_signature_and_pubkey() {
        let w = Wallet::new();
        let base = w.sign_transaction(&"recipient-addr".to_string(), 5);

        for bad_sig in ["", "zz", "abcd", &"00".repeat(64)] {
            let mut t = base.clone();
            t.signature = bad_sig.to_string();
            assert!(
                !Wallet::verify_transaction(&t),
                "signature {bad_sig:?} must fail"
            );
        }
        for bad_pk in ["", "zz", &"00".repeat(32)] {
            let mut t = base.clone();
            t.public_key = bad_pk.to_string();
            assert!(
                !Wallet::verify_transaction(&t),
                "public key {bad_pk:?} must fail"
            );
        }
    }

    #[test]
    fn try_new_from_round_trip_and_mismatch() {
        let w = Wallet::new();
        let d = w.get_wallet_data();
        assert!(Wallet::try_new_from(&d.public_key, &d.private_key, &d.blockchain_address).is_ok());

        let other = Wallet::new().get_wallet_data();
        // Another wallet's public key with this wallet's private key.
        assert!(Wallet::try_new_from(
            &other.public_key,
            &d.private_key,
            &d.blockchain_address
        )
        .is_err());
        // Right keys, another wallet's address.
        assert!(Wallet::try_new_from(
            &d.public_key,
            &d.private_key,
            &other.blockchain_address
        )
        .is_err());
        // Garbage hex.
        assert!(Wallet::try_new_from("zz", &d.private_key, &d.blockchain_address).is_err());
        assert!(Wallet::try_new_from(&d.public_key, "zz", &d.blockchain_address).is_err());
        // Valid but mismatched key pair.
        assert!(Wallet::try_new_from(&d.public_key, &other.private_key, &d.blockchain_address)
            .is_err());
    }

    // Cross-compatibility with the browser wallet (static/wallet.js):
    // keypair generated by WebCrypto (P-256), address derived by the JS
    // Base58Check code, payload built by canonicalPayload(), signature by
    // WebCrypto sign. If any side drifts (JSON shape, DER handling,
    // address derivation), this fails. Regenerate with /tmp/wallet_check.cjs.
    #[test]
    fn browser_signed_transaction_verifies() {
        let public_key = "b944d3235ecf0b97a55b099e4514cc184a6b546c8b5012b723562448a181bd8ecf3e32e8b25527f8dacb8a9eb606a0318fa4a7ec9dc3ba46c77f4018f1fc1001";
        let address = "1AHdBg1qbM5jy8tnoB3qrQ8HpvXcomreKM";
        let tx = Transaction {
            sender: address.to_string(),
            recipeint: "MiniBlockJSProbeRecipient".to_string(),
            amount: 42,
            public_key: public_key.to_string(),
            signature: "4bb2ae96efb20f0900ee76c499188033cda75c828c32753f5db6d4bb0db58c6f68dcf66fb0279b3229afd8cdfb036934845d413807490f3429ce499d2c807c2d".to_string(),
        };
        // Same pubkey must derive the same address on both sides.
        assert_eq!(Wallet::address_for_pubkey_hex(public_key).as_deref(), Some(address));
        // The exact JSON the JS signed (proves byte-identical payloads).
        let unsigned = Transaction { signature: String::new(), ..tx.clone() };
        assert_eq!(
            serde_json::to_string(&unsigned).unwrap(),
            "{\"sender\":\"1AHdBg1qbM5jy8tnoB3qrQ8HpvXcomreKM\",\"recipeint\":\"MiniBlockJSProbeRecipient\",\"amount\":42,\"public_key\":\"b944d3235ecf0b97a55b099e4514cc184a6b546c8b5012b723562448a181bd8ecf3e32e8b25527f8dacb8a9eb606a0318fa4a7ec9dc3ba46c77f4018f1fc1001\",\"signature\":\"\"}"
        );
        assert!(Wallet::verify_transaction(&tx));
    }
}
