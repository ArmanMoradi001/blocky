# Wallet

Source: `src/wallet/wallet.rs`.

## What it does

secp256r1 (P-256) key management: generation, Bitcoin-style addresses, transaction
signing, and signature verification. Every balance-holding identity in the
system is one of these wallets.

## Keys and addresses

- `Wallet::new()` generates a random `SigningKey` (via `SysRng`) and derives
  the `VerifyingKey` + address.
- `private_key_str()` — hex of the 32-byte signing scalar.
- `public_key_str()` — hex of raw `x ‖ y` (64 bytes, no `0x04` prefix).
- `address_str()` — Base58 address derived as:
  1. `SHA256(x ‖ y)` → 2. `RIPEMD160` → 3. prepend version byte `0x00` →
  4. `SHA256` → 5. `SHA256` again → 6. first 4 bytes as checksum appended →
  7. Base58 encode. (Same construction as Bitcoin P2PKH on a P-256 key.)

`GET /get_wallet` returns all three as `WalletData { public_key, private_key,
blockchain_address }`.

## Signing and verification

- `sign_transaction(recipient, amount)` builds `{ sender, recipient, amount,
  public_key, signature: "" }`, signs its JSON serialization with ECDSA, and
  stores the hex signature.
- `verify_transaction(tx)` (all failures return `false`, never panic):
  1. Decode hex signature and rebuild the verifying key from `0x04 ‖ x ‖ y`.
  2. **Bind key to sender**: `generate_address(key)` must equal `tx.sender`,
     so an attacker can't attach their key to someone else's address.
  3. Rebuild the exact signed payload (transaction with empty signature) and
     verify.
- `try_new_from(public_key_hex, private_key_hex, address)` rebuilds a wallet
  from dashboard-supplied keys, checking private→public→address consistency
  and returning descriptive `Err` strings (surfaced as API `400`s). The
  panicking `new_from` wrapper exists but is unused by the API.

## Limitations

- **Private keys leave the wallet.** They travel in plaintext API bodies and
  live in the dashboard DOM — fine for local testing, unacceptable in production.
- **No key storage.** Keys exist in memory only; lose them and the funds are
  unspendable (there is no recovery).
- **No encryption at rest or in transit** (no TLS, no encrypted keystore).
- **P-256, not secp256k1.** Works, but differs from Bitcoin/Ethereum tooling
  if interoperability is ever wanted.
- **Signing covers JSON text**, so any serialization change invalidates old
  signatures; there is no domain separation / replay protection across chains.
