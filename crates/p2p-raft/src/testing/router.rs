use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use memstore::TypeConfig;
use openraft::AnyError;
use parking_lot::Mutex;

use crate::message::RpcRequest;
use crate::message::RpcResponse;
use crate::Dinghy;
use crate::TypeConf;
use crate::RESPONSIVE_INTERVAL;

/// Simulate a network router.
#[derive(Clone)]
pub struct RouterNode<C: TypeConf>
where
    C::SnapshotData: std::fmt::Debug,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
    pub source: C::NodeId,
    pub router: Router<C>,
}

#[derive(Default, Clone, derive_more::Deref)]
pub struct Router<C: TypeConf>(Arc<Mutex<RouterConnections<C>>>)
where
    C::SnapshotData: std::fmt::Debug,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug;

impl<C: TypeConf> Router<C>
where
    C::SnapshotData: std::fmt::Debug,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
    // pub fn node(&self, id: C::NodeId) -> RouterNode<C> {
    //     RouterNode {
    //         source: id,
    //         router: self.clone(),
    //     }
    // }

    /// Create partitions in the network specified by a list of lists of node ids.
    ///
    /// Each list in the list represents a new partition which the specified nodes
    /// will be moved into. Nodes which are not specified in any list will remain
    /// in their current partition, which will be separate from any other partitions
    /// created by this function call.
    pub async fn create_partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = C::NodeId>>,
    ) {
        self.0.lock().create_partitions(partitions);
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
pub struct RouterConnections<C: TypeConf>
where
    C::SnapshotData: std::fmt::Debug,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
    pub targets: BTreeMap<C::NodeId, Dinghy<C>>,
    pub latency: HashMap<(C::NodeId, C::NodeId), u64>,
    pub partitions: BTreeMap<C::NodeId, PartitionId>,
}

pub type PartitionId = u64;

static PARTITION_ID: AtomicU64 = AtomicU64::new(1);

impl<C: TypeConf> RouterConnections<C>
where
    C::SnapshotData: std::fmt::Debug,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
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
            "\n~~~~~~~~~~~~~~~~  PARTITION {:?}  ~~~~~~~~~~~~~~~~",
            self.show_partitions()
        );
    }

    /// Show the current partitions in the network.
    ///
    /// The output of this function, if fed into `create_partitions`,
    /// will recreate the current network state.
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

impl<C: TypeConf> RouterNode<C>
where
    C::SnapshotData: std::fmt::Debug,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
    /// Send raft request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn rpc_request(
        &self,
        to: C::NodeId,
        req: RpcRequest<C>,
    ) -> Result<RpcResponse<C>, AnyError> {
        const LOG_REQ: bool = false;
        const LOG_RESP: bool = false;

        if LOG_REQ {
            // println!("req: {} -> {}: {:?}", self.source, to, &req);
            println!("req: {} -> {}", self.source, to);
        }

        let min = self.source.clone().min(to.clone());
        let max = self.source.clone().max(to.clone());

        let delay = {
            let r = self.router.lock();

            if r.partitions.get(&min) != r.partitions.get(&max) {
                if LOG_RESP {
                    println!("dropped.");
                }

                // can't communicate across partitions
                return Err(AnyError::error("simulated network partition"));
            }

            Duration::from_millis(r.latency.get(&(min, max)).cloned().unwrap_or(0) / 2)
        };

        tokio::time::sleep(delay).await;

        let res = {
            let ding = self.router.lock().targets.get(&to).unwrap().clone();
            ding.handle_request(self.source.clone(), req)
                .await
                .map_err(|e| AnyError::new(&e))?
        };

        tokio::time::sleep(delay).await;

        if LOG_RESP {
            println!("resp {} <- {}", self.source, to);
            // println!("resp {} <- {}: {:?}", self.source, to, res);
        }

        let d = self
            .router
            .lock()
            .targets
            .get(&self.source)
            .unwrap()
            .clone();

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
