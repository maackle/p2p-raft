use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::Mutex;

use openraft::error::Unreachable;
use tokio::sync::oneshot;

use crate::app::RequestTx;
use crate::app::RpcRequest;
use crate::app::RpcResponse;
use crate::decode;
use crate::encode;
use crate::typ::RaftError;
use crate::NodeId;

/// Simulate a network router.
#[derive(Debug, Clone, Default)]
pub struct Router {
    pub targets: Arc<Mutex<BTreeMap<NodeId, RequestTx>>>,
}

impl Router {
    /// Send request `Req` to target node `to`, and wait for response `Result<Resp, RaftError<E>>`.
    pub async fn send(&self, to: NodeId, req: RpcRequest) -> Result<RpcResponse, Unreachable> {
        let (resp_tx, resp_rx) = oneshot::channel();

        tracing::debug!("send to: {}, {:?}", to, req);

        {
            let mut targets = self.targets.lock().unwrap();
            let tx = targets.get_mut(&to).unwrap();

            tx.send((req, resp_tx)).unwrap();
        }

        let res = resp_rx.await.unwrap();
        tracing::debug!("resp from: {}, {:?}", to, res);

        Ok(res)
    }
}
