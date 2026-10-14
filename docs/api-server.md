# API server

Source: `src/api_server/api.rs`, `models.rs`, `views.rs`,
`templates/wallet.html`, `static/wallet.{css,js}`.

## What it does

One `ApiServer` instance serves a single node's HTTP API: wallet dashboard,
transfers, faucet, mining, balances, and chain inspection. State is an
in-memory `BlockChain` behind `Arc<Mutex<…>>`, shared across Actix worker
threads.

```rust
pub struct ApiServer {
    host: String,   // bound IP, e.g. "127.0.0.1"
    port: u16,      // e.g. 5000
    cache: Arc<Mutex<HashMap<String, BlockChain>>>, // "blockchain" key
}
```

A fresh miner wallet is generated at construction and funds block rewards.

## Routes

| Method   | Endpoint             | Handler                  | Description                              |
| -------- | -------------------- | ------------------------ | ---------------------------------------- |
| GET      | `/`                  | `get_index_handler`      | Full chain (human-readable block views)  |
| GET      | `/wallet`            | `get_wallet`             | Wallet dashboard (HTML)                  |
| GET      | `/get_wallet`        | `get_wallet_handler`     | Generate a wallet (JSON)                 |
| GET      | `/amount/{address}`  | `get_amount`             | Confirmed balance                        |
| GET      | `/show_transactions` | `show_transaction`       | Confirmed txs + mempool size             |
| GET/POST | `/mining`            | `mining`                 | Mine a block (confirms mempool)          |
| POST     | `/faucet`            | `faucet`                 | Queue test coins (mine to confirm)       |
| POST     | `/transaction`       | `get_transaction_handler`| Queue a signed transfer (mine to confirm)|

`models.rs` holds all DTOs (`TransactionRequest`, `FaucetRequest`,
`TransferResponse`, `TransactionView`, `BlockView`); on-chain byte addresses
are decoded to strings there so the web UI never sees raw bytes.

## How a transfer works

1. Client signs locally (browser WebCrypto P-256 over the canonical JSON
   payload — see `static/wallet.js: canonicalPayload`) and posts
   `{ public_key, blockchain_address, recipient_address, amount, signature }`
   (amount arrives as a string and is parsed to `u64`). Private keys are
   never sent: the server only verifies.
2. Handler rejects empty fields, `sender == recipient`, and non-positive
   amounts (`400`); `BlockChain::add_transaction` verifies the signature
   (`400` on mismatch).
3. Chain checks confirmed-minus-queued balance; insufficient funds → `422`,
   other problems → `400`.
4. Success → `201` with `{ status: "pending", …, note: "call GET /mining…" }`.
   The transaction is **queued, not confirmed** — mining seals it.

## Wallet dashboard

`GET /wallet` returns one self-contained page. Sources are split but composed
at compile time via `include_str!` (`views::render_wallet_page`), so no extra
routes or runtime file I/O exist:

- `templates/wallet.html` — markup with `{{STYLE}}` / `{{SCRIPT}}` slots
- `static/wallet.css` — dark-theme styling
- `static/wallet.js` — jQuery calls to the endpoints above (unchanged API)

Element IDs (`public_key`, `blockchain_address`,
`input_amount`, `status`, …) are the contract between HTML and JS — renaming
either side breaks the page. The private key lives only in WebCrypto memory:
it is never rendered into the DOM, logged, or posted.

## Limitations

- **In-memory only.** Restarting wipes chain, mempool, and miner identity.
- **Single chain per server**, hardcoded under the `"blockchain"` map key;
  several handlers `unwrap()` on it and would panic if absent.
- **Mining blocks the request.** Proof-of-work runs synchronously inside the
  `GET/POST /mining` handler (and at startup), stalling that worker.
- **Side effects on GET.** `/mining` mutates state over GET, against HTTP
  semantics (kept for dashboard convenience).
- **Plaintext HTTP.** All APIs and peer sync are unencrypted; `GET
  /get_wallet` dispenses full keypairs. Acceptable for local testing, never
  for production — terminate TLS at a reverse proxy and block `/get_wallet`
  beyond loopback.
- **Coarse concurrency.** One big `Mutex` around the whole chain; no
  per-resource locking.
