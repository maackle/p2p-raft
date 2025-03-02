use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;
use std::time::Instant;

use memstore::TypeConfig;
use openraft::RPCTypes;
use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::RaftError;
use openraft::error::Timeout;
use openraft::error::Unreachable;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

use crate::Dinghy;
use crate::RESPONSIVE_INTERVAL;

use super::RaftRequest;
use super::RaftResponse;

/// Simulate a network router.
#[derive(Clone)]
pub struct RouterNode<C: RaftTypeConfig> {
    pub source: C::NodeId,
    pub router: Router<C>,
}

#[derive(Default, Clone, derive_more::Deref)]
pub struct Router<C: RaftTypeConfig>(Arc<Mutex<RouterConnections<C>>>);

impl<C: RaftTypeConfig> Router<C> {
    pub fn node(&self, id: C::NodeId) -> RouterNode<C> {
        RouterNode {
            source: id,
            router: self.clone(),
        }
    }

    pub async fn create_partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = C::NodeId>>,
    ) {
        self.0.lock().await.create_partitions(partitions);
    }
}

impl Router<TypeConfig> {
    pub async fn add_nodes(
        &mut self,
        nodes: impl IntoIterator<Item = u64>,
    ) -> Vec<Dinghy<TypeConfig>> {
        let mut rafts = Vec::new();

        for node in nodes {
            let raft = self.new_raft(node).await;
            rafts.push(raft);
        }
        rafts
    }
}

#[derive(Clone, Default)]
pub struct RouterConnections<C: RaftTypeConfig> {
    pub targets: BTreeMap<C::NodeId, Dinghy<C>>,
    pub latency: HashMap<(C::NodeId, C::NodeId), u64>,
    pub partitions: BTreeMap<C::NodeId, PartitionId>,
}

pub type PartitionId = u64;

static PARTITION_ID: AtomicU64 = AtomicU64::new(1);

impl<C: RaftTypeConfig> RouterConnections<C> {
    pub fn create_partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = C::NodeId>>,
    ) {
        for p in partitions {
            let id = PARTITION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            for n in p {
                self.partitions.insert(n, id);
            }
        }
        println!(
            "\n\n~~~~~~~~~~~~~~~~~~~~~~~~~~~~ PARTITION ~~~~~~~~~~~~~~~~~~~~~~~~~~~~\n{:?}\n\n",
            self.show_partitions()
        );
    }

    pub fn show_partitions(&self) -> BTreeSet<BTreeSet<C::NodeId>> {
        let mut partitions = BTreeMap::new();
        let mut all = self.targets.keys().cloned().collect::<BTreeSet<_>>();
        for (node, id) in self.partitions.iter() {
            all.remove(&node);
            partitions
                .entry(id)
                .or_insert_with(BTreeSet::new)
                .insert(node.clone());
        }
        let mut partitions: BTreeSet<BTreeSet<C::NodeId>> = partitions.values().cloned().collect();
        partitions.insert(all);
        partitions
    }
}

impl<C: RaftTypeConfig> RouterNode<C>
where
    C::SnapshotData: std::fmt::Debug,
    C: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<C>>,
{
    /// Send raft request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn raft_request(
        &self,
        to: C::NodeId,
        req: RaftRequest<C>,
    ) -> Result<RaftResponse<C>, Unreachable> {
        const LOG: bool = false;

        if LOG {
            // println!("req: {} -> {}: {:?}", self.source, to, &req);
            println!("req: {} -> {}", self.source, to);
        }

        let min = self.source.clone().min(to.clone());
        let max = self.source.clone().max(to.clone());

        let delay = {
            let r = self.router.lock().await;

            if r.partitions.get(&min) != r.partitions.get(&max) {
                if LOG {
                    println!("dropped.");
                }

                // can't communicate across partitions
                return Err(Unreachable::new(&std::io::Error::other(
                    "simulated network partition",
                )));
            }
            // if let Some(latency) =  {
            // } else {
            //     return Err(Timeout {
            //         action: RPCTypes::Vote,
            //         id: self.source,
            //         target: to,
            //         timeout: Duration::from_secs(1337),
            //     });
            // }
            Duration::from_millis(r.latency.get(&(min, max)).cloned().unwrap_or(0) / 2)
        };

        tokio::time::sleep(delay).await;

        let res: RaftResponse<C> = {
            let r = self.router.lock().await;
            let ding = r.targets.get(&to).unwrap().clone();
            match req {
                RaftRequest::Append(req) => ding
                    .append_entries(req)
                    .await
                    .map_err(|e| Unreachable::new(&e))?
                    .into(),
                RaftRequest::Snapshot { vote, snapshot } => ding
                    .install_full_snapshot(vote, snapshot)
                    .await
                    .map_err(|e| Unreachable::new(&e))?
                    .into(),
                RaftRequest::Vote(req) => ding
                    .vote(req)
                    .await
                    .map_err(|e| Unreachable::new(&e))?
                    .into(),
            }
        };

        tokio::time::sleep(delay).await;

        if LOG {
            println!("resp {} <- {}", self.source, to);
            // println!("resp {} <- {}: {:?}", self.source, to, res);
        }

        let d = {
            let r = self.router.lock().await;
            r.targets.get(&self.source).unwrap().clone()
        };

        // println!("touching {} <- {}", self.source, to);

        {
            let mut t = d.tracker.lock().await;

            t.touch(&to);

            // TODO: only the leader should do this
            // dbg!(d.id);
            t.handle_absentees(&d, RESPONSIVE_INTERVAL).await;
        }

        Ok(res)
    }
}
