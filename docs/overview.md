# Overview

MiniBlock is a minimal blockchain node in Rust. Three nodes run side by side,
each with its own independent in-memory chain, HTTP API, and UDP gossip socket.

## Architecture

```text
                    ┌────────────────────────────────────┐
                    │            one process             │
                    │                                    │
  127.0.0.1:5000 ───┤ node 1: ApiServer + Membership     │
  127.0.0.2:5000 ───┤ node 2: ApiServer + Membership     │
  127.0.0.3:5000 ───┤ node 3: ApiServer + Membership     │
                    └────────────────────────────────────┘
                        │ HTTP (users)   │ UDP (peers)
                        │ :5000          │ :15000
```

Each node has two faces:

- **HTTP API** (`api_server`) — wallets, transfers, faucet, mining. For humans and clients.
- **UDP membership** (`network::membership`) — SWIM gossip so nodes discover each other. For peers.

The two sides are deliberately separate transports: reliable HTTP for data,
cheap unreliable UDP for failure detection. See `membership.md` for why.

## Money flow

1. `GET /get_wallet` creates a secp256r1 wallet (`wallet.md`).
2. `POST /faucet` queues test coins for the new address (no signature needed —
   faucet transactions use the `THE BLOCKCHAIN` mint identity).
3. `GET /mining` mines a block: queued transactions + a 1-coin miner reward are
   sealed with proof-of-work (`blockchain.md`).
4. `POST /transaction` queues a signed transfer (signature + balance checked
   against confirmed balance minus queued spends).
5. Mine again to confirm. `GET /amount/{address}` reads the confirmed balance.

Nothing is confirmed until it is mined — `/transaction` and `/faucet` only
append to the mempool (`transactions_pool`).

## Key facts

- Consensus: proof-of-work, difficulty 3 leading hex zeros, fixed reward of 1.
- Finality: longest valid chain wins (Nakamoto sync: `GET /chain/height`,
  `GET /chain`, `POST /chain/block`, `GET/POST /chain/pending` + 2s
  background pull, push on mine/tx).
- Storage: everything is in-memory. Restarting a node loses its chain.
- Crypto: ECDSA on `secp256r1` (P-256), Bitcoin-style Base58 addresses.

## Limitations (global)

- **Single process.** All three nodes live in one Tokio process (`network.md`);
  there are no per-node binaries, CLI flags, or config files. They sync over
  HTTP, so splitting them into separate processes later keeps working.
- **No persistence.** In-memory `HashMap` + `Vec` state only.
- **No authentication/encryption** on API or gossip traffic; private keys are
  passed in plaintext JSON (test-grade security throughout).
