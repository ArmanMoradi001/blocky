# Network runner

Source: `src/network/network.rs`, `src/network/mod.rs`.

## What it does

Boots one full node per entry in the `NODES` table. A node is an HTTP API
server plus a UDP membership instance running side by side as Tokio tasks:

```rust
const NODES: [(&str, u16); 3] = [
    ("127.0.0.1", 5000),
    ("127.0.0.2", 5000),
    ("127.0.0.3", 5000),
];
const UDP_PORT_OFFSET: u16 = 10_000; // membership UDP = http_port + offset
```

`run_server()` resolves each node's UDP address, builds its seed list (the
*other* nodes' UDP addresses), and spawns `run_node()` per node. `run_node()`
binds [`Membership`](membership.md) and starts `ApiServer` concurrently:

```rust
tokio::join!(membership.run(), async { server.run().await; });
```

## Address scheme

| Node | HTTP API        | Membership UDP   |
| ---- | --------------- | ---------------- |
| 1    | `127.0.0.1:5000`  | `127.0.0.1:15000`  |
| 2    | `127.0.0.2:5000`  | `127.0.0.2:15000`  |
| 3    | `127.0.0.3:5000`  | `127.0.0.3:15000`  |

Same default HTTP port on distinct loopback IPs mirrors real deployment (every
node uses the default port; identity = IP). The whole `127.0.0.0/8` range is
loopback on Linux, so no extra setup is needed. Each `ApiServer` binds only
its own IP, never `0.0.0.0`.

## How it works

1. `main.rs` (`#[tokio::main]`) calls `network::network::run_server().await`.
2. `run_server()` spawns one Tokio task per node and awaits all handles
   (it returns only if a node task ends, which normally never happens).
3. Inside a node task, the membership gossip loop and the Actix HTTP server
   run concurrently via `join!`.

To change the cluster, edit `NODES` (and keep UDP ports collision-free via
`UDP_PORT_OFFSET`).

## Limitations

- **Hardcoded topology.** No CLI flags, env vars, or config file; adding a
  node means editing source.
- **Single process.** All nodes share one Tokio runtime and fate — killing one
  kills all, so failure-detection paths (suspect/dead) cannot be exercised
  until nodes run as separate processes.
- **Independent chains.** Each `ApiServer` owns a separate `BlockChain` with
  its own miner wallet. No blocks, transactions, or mempools are shared or
  synced (see `overview.md`).
- **No graceful shutdown.** Tasks run until killed; no signal handling.
