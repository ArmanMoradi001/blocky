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

1. Client posts keys + sender + recipient + amount (amount arrives as a
   string and is parsed to `u64`).
2. Handler rejects empty fields, `sender == recipient`, and non-positive
   amounts (`400`); rebuilds the wallet via `try_new_from` (key mismatch →
   `400`); signs and calls `BlockChain::add_transaction`.
3. Chain checks signature and confirmed-minus-queued balance; insufficient
   funds → `422`, other problems → `400`.
4. Success → `201` with `{ status: "pending", …, note: "call GET /mining…" }`.
   The transaction is **queued, not confirmed** — mining seals it.

## Wallet dashboard

`GET /wallet` returns one self-contained page. Sources are split but composed
at compile time via `include_str!` (`views::render_wallet_page`), so no extra
routes or runtime file I/O exist:

- `templates/wallet.html` — markup with `{{STYLE}}` / `{{SCRIPT}}` slots
- `static/wallet.css` — dark-theme styling
- `static/wallet.js` — jQuery calls to the endpoints above (unchanged API)

Element IDs (`public_key`, `private_key`, `blockchain_address`,
`input_amount`, `status`, …) are the contract between HTML and JS — renaming
either side breaks the page.

## Limitations

- **In-memory only.** Restarting wipes chain, mempool, and miner identity.
- **Single chain per server**, hardcoded under the `"blockchain"` map key;
  several handlers `unwrap()` on it and would panic if absent.
- **Mining blocks the request.** Proof-of-work runs synchronously inside the
  `GET/POST /mining` handler (and at startup), stalling that worker.
- **Side effects on GET.** `/mining` mutates state over GET, against HTTP
  semantics (kept for dashboard convenience).
- **Plaintext keys.** Private keys travel in JSON bodies and sit in page DOM;
  acceptable for local testing, never for production.
- **Coarse concurrency.** One big `Mutex` around the whole chain; no
  per-resource locking.
