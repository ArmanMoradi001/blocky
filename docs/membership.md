# Membership (SWIM-style gossip discovery)

Source: `src/network/membership.rs`.

## What it does

Lets nodes discover each other and track liveness without a central registry.
Each node maintains a local member list (`Addr → Alive | Suspect | Dead +
incarnation`) that converges across the group through gossip. This is the
**discovery** layer only — it does not exchange blocks or transactions.

## How it works

Two background loops per node, sharing one UDP socket:

**Probe loop** (every `probe_interval = 1s`):
1. Pick a random `Alive`/`Suspect` member and send `Ping`.
2. `Ack` within `ack_timeout = 300ms` → healthy, done.
3. No `Ack` → send `PingReq` to `k = 2` other members, asking them to ping the
   target on your behalf. Success arrives back as `AckForward`.
4. Direct *and* indirect probes fail → mark the target `Suspect` and gossip it.

**Listener loop**: answers `Ping` with `Ack`, serves `PingReq` helpers, absorbs
`Join`s (replying with the full member list), and merges all incoming gossip.

**Message types** (`SwimMessage`, JSON over datagrams):

| Message      | Purpose                                              |
| ------------ | ---------------------------------------------------- |
| `Join`       | Announce (re)join; seeds reply with `Members`        |
| `Members`    | Full member list, sent to joiners                    |
| `Ping`       | Direct liveness probe (+ piggybacked gossip)         |
| `Ack`        | Probe reply (+ piggybacked gossip)                   |
| `PingReq`    | Ask another member to probe a target for you         |
| `AckForward` | Helper's success report back to the requester        |

**Gossip dissemination**: every message carries the 8 most recent updates, so
news (joins, suspicions, refutations) spreads in ~log-scale rounds with
constant per-node traffic — no broadcasts.

**Refutation via incarnation numbers**: a live node that sees itself suspected
bumps its incarnation and re-announces `Alive`; higher incarnation always wins,
so false positives (slow network, GC pause) heal without operator action.
Unrefuted suspects become `Dead` after `suspect_timeout = 4s`; dead rows are
garbage-collected after 60s so restarted nodes can rejoin with a fresh `Join`.

**Joining**: a node needs only its seeds (the other nodes' UDP addresses, wired
in `network.rs`). It sends `Join` to unknown seeds every tick until they are
known, covering nodes that start in any order.

## Configuration

`MembershipConfig` (defaults in parentheses): `probe_interval` (1s),
`ack_timeout` (300ms), `indirect_k` (2), `suspect_timeout` (4s),
`dead_gc_after` (60s). With 3 nodes only 1 indirect helper exists, so
`indirect_k` is effectively capped by group size.

## Observing it

Run with `RUST_LOG=info cargo run`:

```text
membership: 127.0.0.2:15000 discovered 127.0.0.1:15000 as Alive
membership: 127.0.0.1:15000 knows 3/3 alive: [...]
```

## Limitations

- **No authentication or encryption.** Any process that can send UDP can join,
  suspect others, or inject gossip. Spoofed `Suspect`/`Dead` updates are
  trusted at face value (incarnation-gated). Test-grade only.
- **No transport security / NAT traversal.** Raw UDP on fixed ports; fine on
  loopback/LAN, not internet-ready.
- **Failure paths untestable in-process.** Suspect/dead transitions need a
  node to actually stop, which currently kills the whole process
  (see `network.md`).
- **No persistence.** Membership rebuilds from seeds on every restart.
- **No observability endpoint.** The member list is log-only; there is no
  `GET /peers` yet.
- **Fixed-size gossip window.** Only the 8 latest updates piggyback; under
  heavy churn an update could in principle be evicted before spreading (a
  non-issue at current scale).
