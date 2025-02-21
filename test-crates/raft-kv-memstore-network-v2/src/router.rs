use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use openraft::anyerror;
use openraft::error::Timeout;
use openraft::error::Unreachable;
use openraft::RPCTypes;
use tokio::sync::oneshot;

use crate::app::RequestTx;
use crate::app::RpcRequest;
use crate::app::RpcResponse;
use crate::decode;
use crate::encode;
use crate::typ::RaftError;
use crate::NodeId;
use crate::TypeConfig;

/// Simulate a network router.
#[derive(Debug, Clone)]
pub struct RouterNode {
    pub source: NodeId,
    pub router: Arc<Mutex<Router>>,
}

#[derive(Debug, Clone, Default)]
pub struct Router {
    pub targets: BTreeMap<NodeId, RequestTx>,
    pub connection: HashMap<(NodeId, NodeId), u64>,
}

impl RouterNode {
    /// Send request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn send(
        &self,
        to: NodeId,
        req: RpcRequest,
    ) -> Result<RpcResponse, Timeout<TypeConfig>> {
        let (resp_tx, resp_rx) = oneshot::channel();

        tracing::debug!("send to: {}, {:?}", to, req);

        let min = self.source.min(to);
        let max = self.source.max(to);

        let delay = {
            let mut r = self.router.lock().unwrap();
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
            tx.send((req, resp_tx)).unwrap();
            r.connection.get(&(min, max)).cloned()
        };

        if let Some(delay) = delay {
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }

        let res = resp_rx.await.unwrap();
        tracing::debug!("resp from: {}, {:?}", to, res);

        Ok(res)
    }
}
