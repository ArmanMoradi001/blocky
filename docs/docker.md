# Docker setup

Source: `Dockerfile`, `docker-compose.yml`, `.dockerignore`.

The image is a multi-stage build (Rust builder → `debian:bookworm-slim`
runtime, binary `mini_block`, exposes `5000/tcp + 15000/udp`).
The compose file runs the 3-node cluster as one container per node.

## How it works

Each container runs a **single node** (`BLOCKY_UDP_ADVERTISE` set), instead
of the local-dev default of 3 nodes in one process:

| Variable               | Meaning                                              |
| ---------------------- | ---------------------------------------------------- |
| `BLOCKY_HTTP_HOST`     | HTTP bind host (default `0.0.0.0`)                   |
| `BLOCKY_HTTP_PORT`     | HTTP port (default `5000`)                           |
| `BLOCKY_UDP_BIND`      | UDP socket bind (default `0.0.0.0:<http+10000>`)     |
| `BLOCKY_UDP_ADVERTISE` | Routable self addr, e.g. `node1:15000` (**required**) |
| `BLOCKY_SEEDS`         | Peer UDP addrs, e.g. `node2:15000,node3:15000`        |

Hostnames are resolved at startup, so Compose service names work as
advertise/seed values. Sync derives HTTP peers as `udp_port - 10000`, so
keep the `+10000` offset convention. Without any of these vars the binary
falls back to the loopback 3-node cluster (`BLOCKY_NODES`,
`BLOCKY_BIND_HOST` can adjust that instead).

```bash
docker compose up --build -d
curl localhost:5001/peers          # all 3 UDP addrs alive (~8s to converge)
curl localhost:5001/chain/height
docker compose down
```

## Why Docker is not recommended here

- **No persistence.** Chain state is in-memory; restarting a container
  wipes its ledger. Peers will re-sync you, but container churn buys
  nothing over `cargo run`.
- **Designed as a simulator.** The default 3-nodes-in-one-process exists
  so gossip + Nakamoto sync can be watched on loopback with zero setup.
  Splitting that across containers adds bind-vs-advertise addressing, UDP
  port mapping, and DNS resolution for no new behavior.
- **UDP gossip is NAT-hostile.** Membership advertises dial-back
  addresses (`Membership::bind(bind, advertise, …)`); any misconfiguration
  silently partitions the cluster.
- **Heavy, drift-prone builds.** The Rust release build takes minutes in
  Docker, and the builder toolchain can lag the local one (this already
  broke once: `std::iter::chain` errored on Rust 1.89 while local 1.98
  only warned).
- **Local-dev security posture.** Plain HTTP, key-dispensing
  `/get_wallet`, indulgent faucet — fine on loopback, worse the moment a
  Docker port-mapping exposes it.

Prefer `cargo run` unless you specifically need to exercise the
multi-process gossip path.
