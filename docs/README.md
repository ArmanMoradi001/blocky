# MiniBlock Documentation

Section-by-section guide to the codebase. Start here, then read the section you need.

## Map

| File                  | Covers                                                        |
| --------------------- | ------------------------------------------------------------- |
| `overview.md`         | Architecture, data flow, and how the pieces fit together      |
| `network.md`          | Multi-node runner: one HTTP + one UDP socket per node         |
| `membership.md`       | SWIM-style gossip membership over UDP (peer discovery)        |
| `api-server.md`       | HTTP API: routes, DTOs, validation, wallet dashboard         |
| `blockchain.md`       | Chain, blocks, proof-of-work, mempool, faucet                 |
| `wallet.md`           | Keys, addresses, signing and verification                     |
| `crypto.md`           | Standalone signing demo helper                                |

## Reading order for newcomers

1. `overview.md` — the big picture.
2. `wallet.md` → `blockchain.md` — how money works.
3. `api-server.md` — how the outside world talks to a node.
4. `network.md` → `membership.md` — how nodes find each other.

## Source layout

```text
src/
├── main.rs                  # Tokio entrypoint, boots the network runner
├── api_server/
│   ├── api.rs               # Routing + endpoint handlers
│   ├── models.rs            # Request/response DTOs
│   ├── views.rs             # Wallet page renderer (compile-time includes)
│   ├── templates/wallet.html
│   └── static/              # wallet.css, wallet.js
├── blockchain/
│   ├── blockchain.rs        # Block, BlockChain, mining, mempool
│   └── transaction.rs       # On-chain transaction + binary codec
├── wallet/wallet.rs         # secp256r1 keys, addresses, signing
├── crypto.rs                # Standalone demo helper (unused by the node)
└── network/
    ├── network.rs           # Spawns one node per (IP, port)
    └── membership.rs        # SWIM gossip membership over UDP
```
