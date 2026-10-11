# MiniBlock

A minimal blockchain node in Rust with a wallet dashboard and JSON API, built on [Actix Web](https://actix.rs).

Each node keeps an in-memory chain, supports signed transfers, a test-coin faucet, and proof-of-work mining.

## Features

- ECDSA (`secp256r1`) wallets with hex keys and addresses
- Signed transfers with mempool validation
- Faucet for test coins
- Proof-of-work mining to confirm pending transactions
- Wallet dashboard (`/wallet`) with balance, faucet, mining, and send flows
- Human-readable JSON views for blocks and transactions

## Project structure

```text
src/
├── main.rs                  # Tokio entrypoint, boots the network runner
├── api_server/
│   ├── api.rs               # Routing + endpoint handlers
│   ├── models.rs            # Request/response DTOs
│   ├── views.rs             # Wallet page renderer (compile-time includes)
│   ├── templates/wallet.html
│   └── static/              # wallet.css, wallet.js
├── blockchain/              # Chain, blocks, transactions, mining
├── wallet/                  # Key generation, signing, addresses
└── network/
    └── network.rs           # Spawns one ApiServer per port
```

## Prerequisites

- Rust stable toolchain (`rustup`, `cargo`)

## Run

```bash
cargo run
```

By default three independent nodes start (see `src/network/network.rs`):

- `http://127.0.0.1:5000`
- `http://127.0.0.1:5001`
- `http://127.0.0.1:5002`

> Note: each port holds its own independent in-memory chain. Balances mined on one port are not visible on the others.

With logs:

```bash
RUST_LOG=info cargo run
```

## Wallet dashboard

Open `http://127.0.0.1:5000/wallet` and follow the steps on the page:

1. A wallet is generated for the page load.
2. Get test coins from the faucet.
3. Mine a block to confirm.
4. Send coins, then mine again to confirm.

## API reference

All examples use port `5000`.

| Method | Endpoint              | Description                              |
| ------ | --------------------- | ---------------------------------------- |
| GET    | `/`                   | Full chain (human-readable block views)  |
| GET    | `/wallet`             | Wallet dashboard (HTML)                  |
| GET    | `/get_wallet`         | Generate a wallet (JSON)                 |
| GET    | `/amount/{address}`   | Balance for an address                   |
| GET    | `/show_transactions`  | Confirmed transactions + mempool size    |
| GET/POST | `/mining`           | Mine a block (confirms pending txs)      |
| POST   | `/faucet`             | Queue test coins (mine to confirm)       |
| POST   | `/transaction`        | Queue a signed transfer (mine to confirm)|

### Generate a wallet

```bash
curl http://127.0.0.1:5000/get_wallet
```

```json
{
  "public_key": "...",
  "private_key": "...",
  "blockchain_address": "..."
}
```

### Fund via faucet

```bash
curl -X POST http://127.0.0.1:5000/faucet \
  -H 'Content-Type: application/json' \
  -d '{"recipient_address": "<ADDRESS>", "amount": "100"}'
```

### Mine

```bash
curl http://127.0.0.1:5000/mining
```

### Send

```bash
curl -X POST http://127.0.0.1:5000/transaction \
  -H 'Content-Type: application/json' \
  -d '{
    "private_key": "<HEX>",
    "public_key": "<HEX>",
    "blockchain_address": "<SENDER>",
    "recipient_address": "<RECIPIENT>",
    "amount": "10"
  }'
```

Mine again afterwards to confirm, then check the balance:

```bash
curl http://127.0.0.1:5000/amount/<ADDRESS>
```

## Tech stack

- `actix-web` — HTTP server
- `tokio` — async runtime / multi-node spawning
- `p256` — ECDSA signing
- `sha2` / `ripemd160` / `bs58` — hashing and addresses
- `serde` / `serde_json` — JSON API
