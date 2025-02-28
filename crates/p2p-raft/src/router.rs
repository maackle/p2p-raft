use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use openraft::RPCTypes;
use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::RaftError;
use openraft::error::Timeout;
use openraft::error::Unreachable;
use tokio::sync::Mutex;
use tokio::sync::oneshot;

use crate::Dinghy;
use crate::network::RaftRequest;
use crate::network::RaftResponse;

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

    pub async fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = C::NodeId>>,
    ) {
        self.0.lock().await.partitions(partitions);
    }
}

impl Router<network_impl::TypeConfig> {
    pub async fn add_nodes(
        &mut self,
        nodes: impl IntoIterator<Item = u64>,
    ) -> (
        Vec<Raft<network_impl::TypeConfig>>,
        tokio::task::JoinSet<()>,
    ) {
        let mut rafts = Vec::new();
        let mut js = tokio::task::JoinSet::new();
        let nodes = nodes.into_iter().collect::<Vec<_>>();

        for node in nodes {
            let (raft, app) = crate::newraft::new_raft(node, self.clone()).await;

            js.spawn(async move { app.run().await.unwrap() });
            rafts.push(raft);
        }
        (rafts, js)
    }
}

#[derive(Clone, Default)]
pub struct RouterConnections<C: RaftTypeConfig> {
    pub targets: BTreeMap<C::NodeId, Dinghy<C>>,
    pub latency: HashMap<(C::NodeId, C::NodeId), u64>,
    pub partitions: BTreeMap<C::NodeId, PartitionId>,
}

pub type PartitionId = u64;

impl<C: RaftTypeConfig> RouterConnections<C> {
    pub fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = C::NodeId>>,
    ) {
        for p in partitions {
            let id = rand::random();
            for n in p {
                self.partitions.insert(n, id);
            }
        }
    }
}

impl<C: RaftTypeConfig> RouterNode<C> {
    /// Send raft request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn raft_request(
        &self,
        to: C::NodeId,
        op: RaftRequest<C>,
    ) -> Result<RaftResponse<C>, Unreachable> {
        // println!("req: {} -> {}", self.source, to);

        let min = self.source.clone().min(to.clone());
        let max = self.source.clone().max(to.clone());

        let delay = {
            let r = self.router.lock().await;

            if r.partitions.get(&min) != r.partitions.get(&max) {
                // println!("dropped.");

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
            match op {
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

        // println!("resp {} <- {}", self.source, to);
        // println!("resp {}<-{}, {:?}", self.source, to, res);

        let d = {
            let r = self.router.lock().await;
            r.targets.get(&self.source).unwrap().clone()
        };

        println!("touching {} <- {}", self.source, to);
        d.tracker.lock().await.touch(&to);

        Ok(res)
    }
}
