use rand::rngs::SysRng;
use rand_core::UnwrapErr;
use ripemd160::{Digest as ripDigest, Ripemd160};
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
}
