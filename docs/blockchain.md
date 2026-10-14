# Blockchain

Source: `src/blockchain/blockchain.rs`, `src/blockchain/transaction.rs`.

## What it does

The ledger: an ordered `Vec<Block>` plus a mempool (`transactions_pool`) of
queued, unconfirmed transactions. Balances are computed by scanning the whole
chain — there is no UTXO set or account table.

## Blocks

```rust
pub struct Block {
    nonce: i32,                 // proof-of-work counter
    previous_hash: Vec<u8>,     // hash of the parent block
    time_stamp: u128,           // nanos since UNIX epoch
    transactions: Vec<Vec<u8>>, // serialized on-chain transactions
}
```

- `hash()` = SHA-256 over `nonce || previous_hash || timestamp || txs`.
- Genesis is the deterministic `Block::genesis()` (fixed nonce/timestamp/
  empty txs, shared by all nodes), followed by an immediate `mining()`
  at construction, so a new chain starts with 2 blocks.
- `last_block()` has an off-by-one quirk worth knowing: with more than one
  block it returns the tip, otherwise block 0 (harmless since genesis always
  exists).

## Mining (proof-of-work)

- Difficulty: `DIFFICULTY = 3` — the hex-encoded hash must start with `"000"`.
- Each `mining()` call appends a 1-coin reward to the node's miner address
  (`MINING_REWARD = 1`, sender `THE BLOCKCHAIN`), moves the whole mempool into
  a new block, grinds `nonce` until the difficulty is met, and pushes the block.
- Mining is synchronous and CPU-bound; block time depends on machine speed.

## Transactions

Two shapes exist — do not confuse them:

| Type | Lives in | Fields |
| ---- | -------- | ------ |
| `wallet::Transaction` | API/mempool input | sender, recipient, amount, public_key, signature |
| `blockchain::Transaction` | On-chain storage | sender/recipient as raw bytes, value (`u64`), public key + signature (raw 64 B each; empty for mints) |

`add_transaction()` enforces: non-zero amount, sender ≠ recipient, valid
ECDSA signature (skipped for the `THE BLOCKCHAIN` mint identity), and
`confirmed_balance − already_queued ≥ amount` — this last check is what
prevents double-spending unconfirmed funds. The public key + signature are
persisted with the transaction and **re-verified on every validation path**
(`validate_block`, `validate_chain` replay, mempool merge, block push), so a
peer cannot forge spends from an address it does not own. Repeat sends of
identical sender/recipient/amount are allowed — each carries its own
signature and is an independent spend.

**Binary codec** (`Serialization`): `sender_len(u64 BE) ‖ sender ‖
receiver_len(u64 BE) ‖ receiver ‖ value(u64 BE) ‖ pubkey_len(u64 BE) ‖
pubkey ‖ sig_len(u64 BE) ‖ sig`. `try_deserialization` returns `None` on
truncated/trailing bytes and is what all validation paths use; the panicking
`deserialization` wrapper survives only on print/balance paths over already
accepted data.

**Faucet** (`faucet(recipient, amount)`): mints coins via the
`THE BLOCKCHAIN` identity, bypassing signature and balance checks. Queued like
any other transaction — mine to confirm.

**Balances** (`calculate_total_amount`): full chain scan, `+value` for receipts,
`−value` for sends, returned as `i64` (can go negative only transiently in
checks; enforced non-negative by the mempool rule).

## Limitations

- **No persistence** — chain and mempool live in `Vec`s; restarts lose everything.
- **Crude fork handling** — longest-*valid*-chain rule with signature +
  ledger replay on adopt (`replace_chain_if_longer_valid`); no total-work
  tiebreak, single-block push path checks linkage/PoW/signatures only.
- **Fixed difficulty and reward** — no adjustment algorithm; `3` zeros and
  `1` coin are constants.
- **Panics on bad input** — `block_search` indexing, `Index` impl, and tx
  deserialization use `panic!`/`assert!` instead of `Result`.
- **`block_search` is O(n) with pointer-identity index lookup** and largely
  unused by the API; kept for debugging.
- **Miner address is immutable** per chain instance (`blockchain_address` set
  at construction from a fresh random wallet).
