# MiniBlock

A minimal blockchain node in Rust with a wallet dashboard and JSON API, built on [Actix Web](https://actix.rs).

Each node keeps an in-memory chain, supports signed transfers, a test-coin faucet, and proof-of-work mining.
Nodes discover each other via SWIM-style gossip over UDP and converge on one ledger via Nakamoto longest-valid-chain sync over HTTP.

## Features

- ECDSA (`secp256r1`) wallets with hex keys and addresses
- Signed transfers with mempool validation
- Faucet for test coins
- Proof-of-work mining to confirm pending transactions
- Wallet dashboard (`/wallet`) with balance, faucet, mining, and send flows
- Human-readable JSON views for blocks and transactions
- SWIM-style gossip membership so nodes discover each other over UDP
- Nakamoto sync: longest-valid-chain convergence with mempool gossip
  (background pull every 2s + push on mine/faucet/transaction)
- Deterministic shared genesis block so all nodes start from the same hash
- Peer inspection via `GET /peers`

## Documentation

Section-by-section guides live in [`docs/`](docs/README.md): overview,
network, membership, api-server, blockchain, wallet, crypto.

## Project structure

```text
src/
├── main.rs                  # Tokio entrypoint, boots the network runner
├── api_server/
│   ├── api.rs               # Routing + endpoint handlers (wallet + chain sync)
│   ├── models.rs            # Request/response DTOs (incl. chain sync DTOs)
│   ├── views.rs             # Wallet page renderer (compile-time includes)
│   ├── templates/wallet.html
│   └── static/              # wallet.css, wallet.js
├── blockchain/
│   ├── blockchain.rs        # Block, BlockChain, mining, mempool, chain validation
│   └── transaction.rs       # On-chain transaction + binary codec
├── wallet/wallet.rs         # secp256r1 keys, addresses, signing
├── crypto.rs                # Standalone demo helper (unused by the node)
└── network/
    ├── network.rs           # Spawns one node per (IP, port) + sync loop
    ├── membership.rs        # SWIM gossip membership over UDP
    └── sync.rs              # Nakamoto longest-chain + mempool sync over HTTP
```

## Prerequisites

- Rust stable toolchain (`rustup`, `cargo`)

## Run

```bash
cargo run
```

By default three nodes start in one process (see `src/network/network.rs`):

- `http://127.0.0.1:5000` (UDP `127.0.0.1:15000`)
- `http://127.0.0.2:5000` (UDP `127.0.0.2:15000`)
- `http://127.0.0.3:5000` (UDP `127.0.0.3:15000`)

Each node mines with its own miner wallet, but chains converge via Nakamoto
longest-valid-chain sync: a background loop pulls the tallest same-genesis
peer chain every 2s, and newly mined blocks / pending transactions are pushed
to peers immediately. Ctrl+C shuts the process down gracefully.

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
| GET    | `/chain/height`       | Chain height + tip/genesis hashes (sync) |
| GET    | `/chain`              | Full chain for sync (DTOs)               |
| GET    | `/chain/headers`      | Block headers (`?from=<hash>&limit=N`)   |
| GET    | `/chain/block/{hash}` | One block by hash                        |
| POST   | `/chain/block`        | Push a newly mined block to a peer       |
| GET    | `/chain/pending`      | Peer mempool (raw tx hex)                |
| POST   | `/chain/pending`      | Push mempool entries to a peer           |
| GET    | `/peers`              | SWIM alive peers + self UDP addr         |

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

### Chain sync

Peers converge automatically, but the sync endpoints are plain HTTP:

```bash
curl http://127.0.0.1:5000/chain/height
curl http://127.0.0.1:5000/chain/headers?limit=10
curl http://127.0.0.1:5000/chain/pending
curl http://127.0.0.1:5000/peers
```

`POST /chain/block` only fast-paths a block that directly extends the tip;
anything else returns `409` and the 2s background pull loop fetches the full
longest valid chain instead.

## Tech stack

- `actix-web` — HTTP server
- `tokio` — async runtime / multi-node spawning
- `p256` — ECDSA signing
- `sha2` / `ripemd160` / `bs58` — hashing and addresses
- `serde` / `serde_json` — JSON API
