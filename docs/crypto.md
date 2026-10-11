# Crypto helper

Source: `src/crypto.rs` (`generate_and_sign`).

## What it does

A standalone ECDSA demo: generates a P-256 keypair, signs an arbitrary message,
verifies the signature, and prints the private key, public key, signature, and
verification result as hex to stdout.

## How it relates to the node

It doesn't — nothing in `api_server`, `blockchain`, `wallet`, or `network`
calls it. The wallet module (`wallet.md`) implements its own equivalent logic
tailored to transactions (address binding, JSON payload, non-panicking
verification). This file is a scratchpad/reference for the raw primitives.

## Limitations

- Demo output only; no return value, no reuse.
- Prints private key material to stdout.
- Currently dead code (expect a `never used` warning).
- Candidate for deletion or for conversion into a `#[test]` / example once the
  wallet module is the single source of truth for signing.
