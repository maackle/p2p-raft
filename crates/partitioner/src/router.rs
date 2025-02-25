use itertools::Itertools;
use parking_lot::Mutex;
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::task::JoinSet;

pub type NodeId = u64;
pub type PartitionId = u64;

const TIMEOUT: tokio::time::Duration = tokio::time::Duration::from_secs(1);

#[derive(Debug, Clone, derive_more::Deref)]
pub struct Router<Node>(Arc<Mutex<RouterConnections<Node>>>);

impl<Node> Default for Router<Node> {
    fn default() -> Self {
        Self(Arc::new(Mutex::new(RouterConnections::default())))
    }
}

impl<Node: RouterNode> Router<Node> {
    pub fn add_nodes(&mut self, nodes: impl IntoIterator<Item = (NodeId, Node)>) {
        let mut r = self.0.lock();
        r.nodes.extend(nodes.into_iter().map(|(id, node)| {
            let entry = RouterEntry { node, partition: 0 };
            (id, entry)
        }));
    }

    pub fn with_nodes(&self, f: impl Fn(&Node)) {
        let mut r = self.0.lock();
        for (_, entry) in r.nodes.iter_mut() {
            f(&entry.node);
        }
    }

    pub fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = NodeId>>,
    ) {
        let mut r = self.0.lock();
        r.partitions(partitions);
    }

    pub fn call(
        &self,
        from: NodeId,
        to: NodeId,
        action: impl FnOnce(&Node) + Send + 'static,
    ) -> impl Future<Output = Result<(), tokio::time::error::Elapsed>> + Send {
        let r = self.0.lock();

        let min = from.min(to);
        let max = from.max(to);
        let delay = r
            .latency
            .get(&(min, max))
            .cloned()
            .unwrap_or(tokio::time::Duration::ZERO);

        let source_partition = r.nodes.get(&from).expect("node not found").partition;
        let target = r.nodes.get(&to).expect("node not found").clone();

        tokio::time::timeout(TIMEOUT, async move {
            if source_partition == target.partition {
                tokio::time::sleep(delay / 2).await;
                let ret = action(&target.node);
                tokio::time::sleep(delay / 2).await;
                ret
            } else {
                // ensure that the timeout is triggered
                tokio::time::sleep(TIMEOUT * 2).await;
            }
        })
    }

    pub async fn broadcast(
        &self,
        from: NodeId,
        action: impl FnMut(&Node) + Clone + Send + 'static,
    ) -> JoinSet<(NodeId, Result<(), tokio::time::error::Elapsed>)> {
        let mut js = JoinSet::new();
        for to in self.0.lock().nodes.keys().copied().collect_vec() {
            if to != from {
                let mut action = action.clone();
                let this = self.clone();
                js.spawn(async move { (to, this.call(from, to, move |node| action(node)).await) });
            }
        }
        js
    }
}

#[derive(Debug, Clone)]
pub struct RouterConnections<Node> {
    nodes: BTreeMap<NodeId, RouterEntry<Node>>,
    latency: HashMap<(NodeId, NodeId), tokio::time::Duration>,
}

impl<Node> Default for RouterConnections<Node> {
    fn default() -> Self {
        Self {
            nodes: BTreeMap::new(),
            latency: HashMap::new(),
        }
    }
}

impl<Node: Send> RouterConnections<Node> {
    pub fn nodes(&self) -> impl Iterator<Item = (NodeId, &Node)> {
        self.nodes.iter().map(|(id, entry)| (*id, &entry.node))
    }

    pub fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = NodeId>>,
    ) {
        for p in partitions {
            let id = std::random::random();
            for n in p {
                self.nodes.get_mut(&n).expect("node not found").partition = id;
            }
        }
    }
}

#[derive(Debug, Clone, Default)]
struct RouterEntry<Node> {
    node: Node,
    partition: PartitionId,
}

pub trait RouterNode: Sized + Clone + Send + Sync + 'static {}
