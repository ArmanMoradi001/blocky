//! SWIM-style gossip membership for peer discovery (UDP transport).
//!
//! Each node owns one [`Membership`] and runs two background loops:
//!
//! - `listen_loop` — receives datagrams and answers `Ping` with `Ack`,
//!   serves indirect probes (`PingReq`), and absorbs `Join`s.
//! - `probe_loop` — every `probe_interval` pings a random member; on
//!   timeout it asks `k` other members to probe on its behalf, and only
//!   marks the target `Suspect` if those fail too.
//!
//! Updates (`Alive` / `Suspect` / `Dead` + `incarnation`) piggyback on
//! every message as gossip, so all nodes converge without broadcasts.
//! A live node refutes a false suspicion by bumping its incarnation and
//! re-announcing itself `Alive`. Unrefuted suspects become `Dead` after
//! `suspect_timeout`; dead entries are garbage-collected after
//! `dead_gc_after` so restarted nodes can rejoin via `Join`.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use log::{debug, info, warn};
use rand::prelude::{IndexedRandom, SliceRandom};
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::sync::{Mutex, oneshot};
use tokio::time::timeout;

const MAX_DATAGRAM: usize = 2048;
/// How many recent updates ride along on every message.
const GOSSIP_FANOUT: usize = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MemberStatus {
    Alive,
    Suspect,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberUpdate {
    pub addr: SocketAddr,
    pub status: MemberStatus,
    pub incarnation: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SwimMessage {
    Join {
        sender: SocketAddr,
        incarnation: u64,
    },
    Members {
        sender: SocketAddr,
        members: Vec<MemberUpdate>,
    },
    Ping {
        sender: SocketAddr,
        incarnation: u64,
        gossip: Vec<MemberUpdate>,
    },
    Ack {
        sender: SocketAddr,
        incarnation: u64,
        gossip: Vec<MemberUpdate>,
    },
    PingReq {
        requester: SocketAddr,
        requester_incarnation: u64,
        target: SocketAddr,
    },
    AckForward {
        via: SocketAddr,
        target: SocketAddr,
        incarnation: u64,
    },
}

#[derive(Debug, Clone)]
struct Member {
    status: MemberStatus,
    incarnation: u64,
    changed_at: Instant,
}

#[derive(Debug, Clone)]
pub struct MembershipConfig {
    pub probe_interval: Duration,
    pub ack_timeout: Duration,
    pub indirect_k: usize,
    pub suspect_timeout: Duration,
    pub dead_gc_after: Duration,
}

impl Default for MembershipConfig {
    fn default() -> Self {
        Self {
            probe_interval: Duration::from_secs(1),
            ack_timeout: Duration::from_millis(300),
            indirect_k: 2,
            suspect_timeout: Duration::from_secs(4),
            dead_gc_after: Duration::from_secs(60),
        }
    }
}

pub struct Membership {
    self_addr: SocketAddr,
    seeds: Vec<SocketAddr>,
    config: MembershipConfig,
    socket: Arc<UdpSocket>,
    incarnation: Mutex<u64>,
    members: Mutex<HashMap<SocketAddr, Member>>,
    /// Probe waiters per target address; completed when any Ack arrives.
    pending: Mutex<HashMap<SocketAddr, Vec<oneshot::Sender<u64>>>>,
    /// Recently changed updates, piggybacked on outgoing messages.
    recent: Mutex<VecDeque<MemberUpdate>>,
}

impl Membership {
    /// Bind the UDP socket and register self as `Alive(0)`.
    pub async fn bind(
        self_addr: SocketAddr,
        seeds: Vec<SocketAddr>,
        config: MembershipConfig,
    ) -> Arc<Self> {
        let socket = UdpSocket::bind(self_addr)
            .await
            .expect("membership: failed to bind UDP socket");
        let this = Arc::new(Self {
            self_addr,
            seeds,
            config,
            socket: Arc::new(socket),
            incarnation: Mutex::new(0),
            members: Mutex::new(HashMap::from([(
                self_addr,
                Member {
                    status: MemberStatus::Alive,
                    incarnation: 0,
                    changed_at: Instant::now(),
                },
            )])),
            pending: Mutex::new(HashMap::new()),
            recent: Mutex::new(VecDeque::new()),
        });
        info!("membership: {self_addr} listening, seeds={:?}", this.seeds);
        this
    }

    /// Run listener + prober until the process ends.
    pub async fn run(self: &Arc<Self>) {
        tokio::join!(self.clone().listen_loop(), self.clone().probe_loop());
    }

    /// Snapshot of currently known members (including self).
    pub async fn member_list(&self) -> Vec<MemberUpdate> {
        let members = self.members.lock().await;
        members
            .iter()
            .map(|(addr, m)| MemberUpdate {
                addr: *addr,
                status: m.status,
                incarnation: m.incarnation,
            })
            .collect()
    }

    pub fn self_addr(&self) -> SocketAddr {
        self.self_addr
    }

    /// UDP addresses of Alive peers (excluding self) — used by chain sync to
    /// derive HTTP peer addresses (`udp_port - UDP_PORT_OFFSET`).
    pub async fn alive_addrs(&self) -> Vec<SocketAddr> {
        let members = self.members.lock().await;
        members
            .iter()
            .filter(|(addr, m)| {
                **addr != self.self_addr && m.status == MemberStatus::Alive
            })
            .map(|(addr, _)| *addr)
            .collect()
    }

    // -- messaging ------------------------------------------------------

    async fn send_to(&self, target: SocketAddr, msg: &SwimMessage) {
        match serde_json::to_vec(msg) {
            Ok(bytes) => {
                if let Err(e) = self.socket.send_to(&bytes, target).await {
                    debug!("membership: {} send to {target} failed: {e}", self.self_addr);
                }
            }
            Err(e) => debug!("membership: {} encode failed: {e}", self.self_addr),
        }
    }

    async fn gossip_snapshot(&self) -> Vec<MemberUpdate> {
        self.recent.lock().await.iter().cloned().collect()
    }

    async fn push_update(&self, update: MemberUpdate) {
        let mut recent = self.recent.lock().await;
        recent.push_back(update);
        while recent.len() > GOSSIP_FANOUT {
            recent.pop_front();
        }
    }

    /// Merge one update into the local view. Returns true if it changed us.
    async fn merge_update(&self, update: MemberUpdate) -> bool {
        if update.addr == self.self_addr {
            // Someone suspects us (or worse): refute by bumping incarnation.
            if update.status != MemberStatus::Alive {
                let mut inc = self.incarnation.lock().await;
                if update.incarnation >= *inc {
                    *inc += 1;
                    let new_inc = *inc;
                    drop(inc);
                    let mut members = self.members.lock().await;
                    if let Some(me) = members.get_mut(&self.self_addr) {
                        me.status = MemberStatus::Alive;
                        me.incarnation = new_inc;
                        me.changed_at = Instant::now();
                    }
                    drop(members);
                    self.push_update(MemberUpdate {
                        addr: self.self_addr,
                        status: MemberStatus::Alive,
                        incarnation: new_inc,
                    })
                    .await;
                    info!(
                        "membership: {} refuted {:?} with incarnation {new_inc}",
                        self.self_addr, update.status
                    );
                    return true;
                }
            }
            return false;
        }

        let mut members = self.members.lock().await;
        match members.get(&update.addr) {
            None => {
                if update.status == MemberStatus::Dead {
                    return false;
                }
                members.insert(
                    update.addr,
                    Member {
                        status: update.status,
                        incarnation: update.incarnation,
                        changed_at: Instant::now(),
                    },
                );
                drop(members);
                self.push_update(update.clone()).await;
                info!(
                    "membership: {} discovered {} as {:?}",
                    self.self_addr, update.addr, update.status
                );
                true
            }
            Some(current) => {
                let newer = update.incarnation > current.incarnation;
                let same_gen_suspicion = update.incarnation == current.incarnation
                    && current.status == MemberStatus::Alive
                    && update.status != MemberStatus::Alive;
                if newer || same_gen_suspicion {
                    let changed = current.status != update.status
                        || current.incarnation != update.incarnation;
                    members.insert(
                        update.addr,
                        Member {
                            status: update.status,
                            incarnation: update.incarnation,
                            changed_at: Instant::now(),
                        },
                    );
                    drop(members);
                    if changed {
                        self.push_update(update.clone()).await;
                        if update.status != MemberStatus::Alive {
                            warn!(
                                "membership: {} marks {} as {:?}",
                                self.self_addr, update.addr, update.status
                            );
                        } else {
                            info!(
                                "membership: {} marks {} alive (incarnation {})",
                                self.self_addr, update.addr, update.incarnation
                            );
                        }
                    }
                    changed
                } else {
                    false
                }
            }
        }
    }

    async fn merge_gossip(&self, gossip: Vec<MemberUpdate>) {
        for update in gossip {
            self.merge_update(update).await;
        }
    }

    /// Wake everyone waiting on an Ack from `addr`.
    async fn complete_waiters(&self, addr: SocketAddr, incarnation: u64) {
        let mut pending = self.pending.lock().await;
        if let Some(waiters) = pending.remove(&addr) {
            for tx in waiters {
                let _ = tx.send(incarnation);
            }
        }
    }

    // -- listener -------------------------------------------------------

    async fn listen_loop(self: Arc<Self>) {
        let mut buf = vec![0u8; MAX_DATAGRAM];
        loop {
            let (n, from) = match self.socket.recv_from(&mut buf).await {
                Ok(v) => v,
                Err(e) => {
                    debug!("membership: {} recv failed: {e}", self.self_addr);
                    continue;
                }
            };
            let msg: SwimMessage = match serde_json::from_slice(&buf[..n]) {
                Ok(m) => m,
                Err(_) => {
                    debug!("membership: {} dropped malformed datagram from {from}", self.self_addr);
                    continue;
                }
            };
            self.clone().handle_message(msg, from).await;
        }
    }

    async fn handle_message(self: Arc<Self>, msg: SwimMessage, _from: SocketAddr) {
        match msg {
            SwimMessage::Join { sender, incarnation } => {
                // A (re)joining node is alive by definition; reset Dead rows.
                {
                    let mut members = self.members.lock().await;
                    members.insert(
                        sender,
                        Member {
                            status: MemberStatus::Alive,
                            incarnation,
                            changed_at: Instant::now(),
                        },
                    );
                }
                self.push_update(MemberUpdate {
                    addr: sender,
                    status: MemberStatus::Alive,
                    incarnation,
                })
                .await;
                info!("membership: {} accepted join from {sender}", self.self_addr);
                let members = self.member_list().await;
                self.send_to(
                    sender,
                    &SwimMessage::Members {
                        sender: self.self_addr,
                        members,
                    },
                )
                .await;
            }
            SwimMessage::Members { sender, members } => {
                self.merge_update(MemberUpdate {
                    addr: sender,
                    status: MemberStatus::Alive,
                    incarnation: 0,
                })
                .await;
                // Merge at face value, but never let a bulk reply resurrect
                // our own refuted incarnation: merge_update guards self.
                for update in members {
                    self.merge_update(update).await;
                }
            }
            SwimMessage::Ping {
                sender,
                incarnation,
                gossip,
            } => {
                self.merge_update(MemberUpdate {
                    addr: sender,
                    status: MemberStatus::Alive,
                    incarnation,
                })
                .await;
                self.merge_gossip(gossip).await;
                let my_inc = *self.incarnation.lock().await;
                self.send_to(
                    sender,
                    &SwimMessage::Ack {
                        sender: self.self_addr,
                        incarnation: my_inc,
                        gossip: self.gossip_snapshot().await,
                    },
                )
                .await;
            }
            SwimMessage::Ack {
                sender,
                incarnation,
                gossip,
            } => {
                self.merge_update(MemberUpdate {
                    addr: sender,
                    status: MemberStatus::Alive,
                    incarnation,
                })
                .await;
                self.merge_gossip(gossip).await;
                self.complete_waiters(sender, incarnation).await;
            }
            SwimMessage::PingReq {
                requester,
                requester_incarnation,
                target,
            } => {
                self.merge_update(MemberUpdate {
                    addr: requester,
                    status: MemberStatus::Alive,
                    incarnation: requester_incarnation,
                })
                .await;
                // Probe on the requester's behalf; forward success.
                let this = self.clone();
                tokio::spawn(async move {
                    let (tx, rx) = oneshot::channel();
                    {
                        let mut pending = this.pending.lock().await;
                        pending.entry(target).or_default().push(tx);
                    }
                    this.send_to(
                        target,
                        &SwimMessage::Ping {
                            sender: this.self_addr,
                            incarnation: *this.incarnation.lock().await,
                            gossip: this.gossip_snapshot().await,
                        },
                    )
                    .await;
                    if let Ok(inc) =
                        timeout(this.config.ack_timeout, async { rx.await.unwrap_or(0) }).await
                    {
                        this.send_to(
                            requester,
                            &SwimMessage::AckForward {
                                via: this.self_addr,
                                target,
                                incarnation: inc,
                            },
                        )
                        .await;
                    }
                });
            }
            SwimMessage::AckForward {
                via: _,
                target,
                incarnation,
            } => {
                self.merge_update(MemberUpdate {
                    addr: target,
                    status: MemberStatus::Alive,
                    incarnation,
                })
                .await;
                self.complete_waiters(target, incarnation).await;
            }
        }
    }

    // -- prober ---------------------------------------------------------

    async fn probe_loop(self: Arc<Self>) {
        // Announce ourselves to seeds; re-announce until they know us
        // (covers nodes that start in any order).
        let mut ticks: u64 = 0;
        loop {
            ticks += 1;
            self.rejoin_unknown_seeds().await;
            self.probe_once().await;
            self.check_suspects().await;
            if ticks % 10 == 0 {
                let list = self.member_list().await;
                let alive = list
                    .iter()
                    .filter(|m| m.status == MemberStatus::Alive)
                    .count();
                info!(
                    "membership: {} knows {}/{} alive: {:?}",
                    self.self_addr,
                    alive,
                    list.len(),
                    list.iter().map(|m| m.addr).collect::<Vec<_>>()
                );
            }
            tokio::time::sleep(self.config.probe_interval).await;
        }
    }

    async fn rejoin_unknown_seeds(&self) {
        let members = self.members.lock().await;
        let missing: Vec<SocketAddr> = self
            .seeds
            .iter()
            .cloned()
            .filter(|s| *s != self.self_addr && !members.contains_key(s))
            .collect();
        drop(members);
        let my_inc = *self.incarnation.lock().await;
        for seed in missing {
            self.send_to(
                seed,
                &SwimMessage::Join {
                    sender: self.self_addr,
                    incarnation: my_inc,
                },
            )
            .await;
        }
    }

    fn pick_probe_target(
        members: &HashMap<SocketAddr, Member>,
        self_addr: SocketAddr,
    ) -> Option<SocketAddr> {
        let candidates: Vec<SocketAddr> = members
            .iter()
            .filter(|(addr, m)| {
                **addr != self_addr
                    && (m.status == MemberStatus::Alive || m.status == MemberStatus::Suspect)
            })
            .map(|(addr, _)| *addr)
            .collect();
        candidates.choose(&mut rand::rng()).cloned()
    }

    async fn wait_for_ack(&self, target: SocketAddr, d: Duration) -> Option<u64> {
        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            // Drop waiters whose receivers already went away.
            if let Some(waiters) = pending.get_mut(&target) {
                waiters.retain(|s| !s.is_closed());
            }
            pending.entry(target).or_default().push(tx);
        }
        timeout(d, async { rx.await.unwrap_or(0) }).await.ok()
    }

    async fn probe_once(&self) {
        let target = {
            let members = self.members.lock().await;
            Self::pick_probe_target(&members, self.self_addr)
        };
        let Some(target) = target else { return };

        let my_inc = *self.incarnation.lock().await;
        self.send_to(
            target,
            &SwimMessage::Ping {
                sender: self.self_addr,
                incarnation: my_inc,
                gossip: self.gossip_snapshot().await,
            },
        )
        .await;
        if self.wait_for_ack(target, self.config.ack_timeout).await.is_some() {
            return;
        }

        // Direct probe failed: ask k others to probe on our behalf.
        let helpers = {
            let members = self.members.lock().await;
            let mut candidates: Vec<SocketAddr> = members
                .iter()
                .filter(|(addr, m)| {
                    **addr != self.self_addr
                        && **addr != target
                        && m.status == MemberStatus::Alive
                })
                .map(|(addr, _)| *addr)
                .collect();
            candidates.shuffle(&mut rand::rng());
            candidates.truncate(self.config.indirect_k);
            candidates
        };
        for helper in &helpers {
            self.send_to(
                *helper,
                &SwimMessage::PingReq {
                    requester: self.self_addr,
                    requester_incarnation: my_inc,
                    target,
                },
            )
            .await;
        }
        if !helpers.is_empty()
            && self
                .wait_for_ack(target, self.config.ack_timeout)
                .await
                .is_some()
        {
            debug!("membership: {} reached {target} indirectly", self.self_addr);
            return;
        }

        // Direct + indirect failed: suspect (gossiped to the group).
        let mut members = self.members.lock().await;
        if let Some(m) = members.get_mut(&target) {
            if m.status == MemberStatus::Alive {
                m.status = MemberStatus::Suspect;
                m.changed_at = Instant::now();
                let update = MemberUpdate {
                    addr: target,
                    status: MemberStatus::Suspect,
                    incarnation: m.incarnation,
                };
                drop(members);
                self.push_update(update).await;
                warn!("membership: {} suspects {target}", self.self_addr);
            }
        }
    }

    /// Promote long-unrefuted suspects to Dead; drop ancient Dead rows so
    /// restarted nodes can rejoin cleanly.
    async fn check_suspects(&self) {
        let now = Instant::now();
        let mut members = self.members.lock().await;
        let mut dead: Vec<MemberUpdate> = vec![];
        members.retain(|addr, m| {
            if m.status == MemberStatus::Dead && now - m.changed_at > self.config.dead_gc_after {
                debug!("membership: {} forgetting dead {addr}", self.self_addr);
                return false;
            }
            true
        });
        for (addr, m) in members.iter_mut() {
            if m.status == MemberStatus::Suspect && now - m.changed_at > self.config.suspect_timeout
            {
                m.status = MemberStatus::Dead;
                m.changed_at = now;
                dead.push(MemberUpdate {
                    addr: *addr,
                    status: MemberStatus::Dead,
                    incarnation: m.incarnation,
                });
            }
        }
        drop(members);
        for update in dead {
            warn!(
                "membership: {} declares {} dead",
                self.self_addr, update.addr
            );
            self.push_update(update).await;
        }
    }
}
