use rand::rngs::SysRng;
use rand_core::UnwrapErr;

use p256::ecdsa::{
    Signature, SigningKey, VerifyingKey,
    signature::{Signer, Verifier},
};

use hex;

pub fn generate_and_sign(message: &[u8]) {
    let mut rng = UnwrapErr(SysRng);

    let signing_key = SigningKey::random(&mut rng);

    let verifying_key = VerifyingKey::from(&signing_key);

    let private_key_hex = hex::encode(signing_key.to_bytes());

    let public_key_hex = hex::encode(verifying_key.to_sec1_point(false).as_bytes());

    let signature: Signature = signing_key.sign(message);

    let signature_hex = hex::encode(signature.to_bytes());

    let result = verifying_key.verify(message, &signature);

    println!("Private Key:");
    println!("{private_key_hex}");

    println!("\nPublic Key:");
    println!("{public_key_hex}");

    println!("\nSignature:");
    println!("{signature_hex}");

    println!("\nVerification:");
    println!("{}", result.is_ok());
}
