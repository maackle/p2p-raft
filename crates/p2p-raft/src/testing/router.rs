use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::Duration;

use super::NodeId;
use super::TypeConfig;
use parking_lot::Mutex;

use crate::config::Config;
use crate::message::Request;
use crate::message::Response;
use crate::signal::SignalSender;

type P2pRaft = crate::P2pRaft<TypeConfig, RouterNode>;

/// Simulate a network router.
#[derive(Clone)]
pub struct RouterNode {
    pub source: NodeId,
    pub router: Router,
}

#[derive(Clone, derive_more::Deref, Default)]
pub struct Router {
    #[deref]
    pub connections: Arc<Mutex<RouterConnections>>,
    pub config: Arc<Config>,
    signal_tx: Option<SignalSender<TypeConfig>>,
}

impl Router {
    pub fn new(config: Config, signal_tx: Option<SignalSender<TypeConfig>>) -> Self {
        Self {
            connections: Arc::new(Mutex::new(RouterConnections::default())),
            config: Arc::new(config),
            signal_tx,
        }
    }

    pub fn rafts(&self) -> Vec<P2pRaft> {
        self.lock().targets.values().cloned().collect()
    }

    pub async fn initialize_nodes(&self) {
        let rafts = self.rafts();
        let all_ids = rafts.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
        for (_, raft) in rafts.iter().enumerate() {
            let _ = tokio::spawn(raft.clone().chore_loop());
            raft.initialize(all_ids.clone()).await.unwrap();
            println!("initialized {}.", raft.id);
        }
    }

    pub async fn natural_startup(&self, delay: Duration) {
        let rafts = self.rafts();
        let all_ids = rafts.iter().map(|r| r.id.clone()).collect::<Vec<_>>();

        for (n, raft) in rafts.iter().enumerate() {
            let _ = tokio::spawn(raft.clone().chore_loop());
            let ids = all_ids[0..=n].to_vec();
            raft.initialize(ids.clone()).await.unwrap();
            raft.broadcast_join(ids.clone()).await.unwrap();
            println!("initialized {}.", raft.id);
            tokio::time::sleep(delay).await;
        }
    }

    /// Create partitions in the network specified by a list of lists of node ids.
    ///
    /// Each list in the list represents a new partition which the specified nodes
    /// will be moved into. Nodes which are not specified in any list will remain
    /// in their current partition, which will be separate from any other partitions
    /// created by this function call.
    pub async fn create_partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = NodeId>>,
    ) {
        self.lock().create_partitions(partitions);
    }
}

impl Router {
    pub async fn add_nodes(&mut self, nodes: impl IntoIterator<Item = u64>) -> Vec<P2pRaft> {
        let mut rafts = Vec::new();

        for node in nodes {
            let config = self.config.clone();
            let raft = self.new_raft(node, config, self.signal_tx.clone()).await;
            rafts.push(raft);
        }
        rafts
    }
}

#[derive(Clone, Default)]
pub struct RouterConnections {
    pub targets: BTreeMap<NodeId, P2pRaft>,
    pub latency: HashMap<(NodeId, NodeId), u64>,
    pub partitions: BTreeMap<NodeId, PartitionId>,
}

pub type PartitionId = u64;

static PARTITION_ID: AtomicU64 = AtomicU64::new(1);

impl RouterConnections {
    pub fn create_partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = NodeId>>,
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
    pub fn show_partitions(&self) -> BTreeSet<BTreeSet<NodeId>> {
        let mut partitions = BTreeMap::new();
        let mut all = self.targets.keys().cloned().collect::<BTreeSet<_>>();
        for (node, id) in self.partitions.iter() {
            all.remove(&node);
            partitions
                .entry(id)
                .or_insert_with(BTreeSet::new)
                .insert(node.clone());
        }
        let mut partitions: BTreeSet<BTreeSet<NodeId>> = partitions.values().cloned().collect();
        partitions.insert(all);
        partitions
    }
}

impl RouterNode {
    /// Route raft request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn route(
        &self,
        to: NodeId,
        req: Request<TypeConfig>,
    ) -> anyhow::Result<Response<TypeConfig>> {
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
                return Err(anyhow::anyhow!("simulated network partition"));
            }

            Duration::from_millis(r.latency.get(&(min, max)).cloned().unwrap_or(0) / 2)
        };

        tokio::time::sleep(delay).await;

        let res = {
            let ding = self.router.lock().targets.get(&to).unwrap().clone();
            match req {
                Request::P2p(p2p_req) => ding
                    .handle_p2p_request(self.source.clone(), p2p_req)
                    .await?
                    .into(),
                Request::Raft(raft_req) => ding
                    .handle_raft_request(self.source.clone(), raft_req)
                    .await?
                    .into(),
            }
        };

        tokio::time::sleep(delay).await;

        if LOG_RESP {
            println!("resp {} <- {}", self.source, to);
            // println!("resp {} <- {}: {:?}", self.source, to, res);
        }

        let r = self
            .router
            .lock()
            .targets
            .get(&self.source)
            .unwrap()
            .clone();

        // println!("touching {} <- {}", self.source, to);

        {
            let mut t = r.tracker.lock().await;
            t.touch(&to);
            t.handle_absentees(&r, self.router.config.p2p_config.responsive_interval)
                .await;
        }

        Ok(res)
    }
}
