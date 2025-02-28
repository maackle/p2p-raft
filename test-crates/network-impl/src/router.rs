use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use openraft::error::Timeout;
use openraft::RPCTypes;
use rand::Rng;
use tokio::sync::oneshot;

use crate::app::RaftRequest;
use crate::app::RequestTx;
use crate::app::RpcRequest;
use crate::app::RpcResponse;

use crate::typ::Raft;
use crate::NodeId;
use crate::TypeConfig;

/// Simulate a network router.
#[derive(Debug, Clone)]
pub struct RouterNode {
    pub source: NodeId,
    pub router: Router,
}

#[derive(Debug, Default, Clone, derive_more::Deref)]
pub struct Router(Arc<Mutex<RouterConnections>>);

impl Router {
    pub async fn add_nodes(
        &mut self,
        nodes: impl IntoIterator<Item = NodeId>,
    ) -> (Vec<Raft>, tokio::task::JoinSet<()>) {
        let mut rafts = Vec::new();
        let mut js = tokio::task::JoinSet::new();
        let nodes = nodes.into_iter().collect::<Vec<_>>();

        for node in nodes {
            let (raft, app) = crate::new_raft(node, self.clone()).await;

            js.spawn(async move { app.run().await.unwrap() });
            rafts.push(raft);
        }
        (rafts, js)
    }

    pub fn node(&self, id: NodeId) -> RouterNode {
        RouterNode {
            source: id,
            router: self.clone(),
        }
    }

    pub fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = NodeId>>,
    ) {
        self.0.lock().unwrap().partitions(partitions);
    }
}

#[derive(Debug, Clone, Default)]
pub struct RouterConnections {
    pub targets: BTreeMap<NodeId, RequestTx>,
    pub latency: HashMap<(NodeId, NodeId), u64>,
    pub partitions: BTreeMap<NodeId, PartitionId>,
}

pub type PartitionId = u64;

impl RouterConnections {
    pub fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = NodeId>>,
    ) {
        for p in partitions {
            let id = rand::rng().random();
            for n in p {
                self.partitions.insert(n, id);
            }
        }
    }
}

impl RouterNode {
    /// Send raft request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn raft_request(
        &self,
        to: NodeId,
        req: RaftRequest,
    ) -> Result<RpcResponse, Timeout<TypeConfig>> {
        let (resp_tx, resp_rx) = oneshot::channel();

        // println!("req: {} -> {}", self.source, to);

        let min = self.source.min(to);
        let max = self.source.max(to);

        let delay = {
            let mut r = self.router.lock().unwrap();

            if r.partitions.get(&min) != r.partitions.get(&max) {
                // println!("dropped.");

                // can't communicate across partitions
                return Err(Timeout {
                    action: RPCTypes::Vote,
                    id: self.source,
                    target: to,
                    timeout: Duration::from_secs(1337),
                });
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
            let tx = r.targets.get_mut(&to).unwrap();
            tx.send((RpcRequest::Raft(req), resp_tx)).unwrap();
            r.latency.get(&(min, max)).cloned()
        };

        if let Some(delay) = delay {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        let res = resp_rx.await.unwrap();
        // println!("resp {} <- {}", self.source, to);
        // println!("resp {}<-{}, {:?}", self.source, to, res);

        todo!("touch tracker");

        Ok(res)
    }
}
