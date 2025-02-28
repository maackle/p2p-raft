use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;
use std::time::Instant;

use openraft::RPCTypes;
use openraft::Raft;
use openraft::RaftTypeConfig;
use openraft::error::Timeout;
use tokio::sync::oneshot;

use crate::Dinghy;

/// Simulate a network router.
#[derive(Clone)]
pub struct RouterNode<C: RaftTypeConfig> {
    pub source: C::NodeId,
    pub router: Router<C>,
}

#[derive(Default, Clone, derive_more::Deref)]
pub struct Router<C: RaftTypeConfig>(Arc<Mutex<RouterConnections<C>>>);

impl<C: RaftTypeConfig> Router<C> {
    pub async fn add_nodes(
        &mut self,
        nodes: impl IntoIterator<Item = C::NodeId>,
    ) -> (Vec<Raft<C>>, tokio::task::JoinSet<()>) {
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

    pub fn node(&self, id: C::NodeId) -> RouterNode<C> {
        RouterNode {
            source: id,
            router: self.clone(),
        }
    }

    pub fn partitions(
        &mut self,
        partitions: impl IntoIterator<Item = impl IntoIterator<Item = C::NodeId>>,
    ) {
        self.0.lock().unwrap().partitions(partitions);
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

// pub fn request<R>(&self, to: NodeId, f: impl FnOnce(&Dinghy<C>) -> R) -> R {
//     let mut r = self.0.lock().unwrap();
//     let tx = r.targets.get_mut(&to).unwrap();
//     f(tx)
// }

impl<C: RaftTypeConfig> RouterNode<C> {
    /// Send raft request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn raft_request(
        &self,
        to: C::NodeId,
        f: impl FnOnce(&Dinghy<C>) -> R,
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
