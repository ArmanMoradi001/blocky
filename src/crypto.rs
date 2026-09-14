use rand::rngs::SysRng;
use rand_core::UnwrapErr;

use p256::ecdsa::{
    signature::{Signer, Verifier},
    Signature,
    SigningKey,
    VerifyingKey,
};

use hex;


pub fn generate_and_sign(message: &[u8]) {
   
    let mut rng = UnwrapErr(SysRng);

    let signing_key = SigningKey::random(&mut rng);


    // --------------------------------------------------
    // 2. Get public key
    // --------------------------------------------------

    let verifying_key = VerifyingKey::from(&signing_key);


    // --------------------------------------------------
    // 3. Encode private key as hex
    // --------------------------------------------------

    let private_key_hex = hex::encode(signing_key.to_bytes());


    // --------------------------------------------------
    // 4. Encode public key as hex
    // --------------------------------------------------

    // SEC1 uncompressed public key:
    // 04 || X || Y
    let public_key_hex =
        hex::encode(verifying_key.to_sec1_point(false).as_bytes());


    // --------------------------------------------------
    // 5. Sign message
    // --------------------------------------------------

    let signature: Signature = signing_key.sign(message);


    // --------------------------------------------------
    // 6. Encode signature as hex
    // --------------------------------------------------

    // ECDSA signature in raw form:
    // r || s
    let signature_hex = hex::encode(signature.to_bytes());


    // --------------------------------------------------
    // 7. Verify signature
    // --------------------------------------------------

    let result = verifying_key.verify(message, &signature);


    // --------------------------------------------------
    // 8. Print everything
    // --------------------------------------------------

    println!("Private Key:");
    println!("{private_key_hex}");

    println!("\nPublic Key:");
    println!("{public_key_hex}");

    println!("\nSignature:");
    println!("{signature_hex}");

    println!("\nVerification:");
    println!("{}", result.is_ok());
}
