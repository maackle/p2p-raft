use std::fmt::Debug;
use std::future::Future;
use std::time::Duration;

use memstore::NodeId;
use memstore::TypeConfig;
use openraft::OptionalSend;
use openraft::RPCTypes;
use openraft::RaftNetworkFactory;
use openraft::RaftTypeConfig;
use openraft::Snapshot;
use openraft::Vote;
use openraft::error::RPCError;
use openraft::error::ReplicationClosed;
use openraft::error::StreamingError;
use openraft::error::Timeout;
use openraft::impls::OneshotResponder;
use openraft::network::RPCOption;
use openraft::network::v2::RaftNetworkV2;
use openraft::raft::AppendEntriesRequest;
use openraft::raft::AppendEntriesResponse;
use openraft::raft::SnapshotResponse;
use openraft::raft::VoteRequest;
use openraft::raft::VoteResponse;

use crate::message::P2pRequest;
use crate::message::RaftRequest;
use crate::network::P2pNetwork;

use super::Router;
use super::router::RouterNode;

pub struct Connection<C: RaftTypeConfig> {
    router: RouterNode<C>,
    target: C::NodeId,
}

impl RaftNetworkFactory<TypeConfig> for RouterNode<TypeConfig> {
    type Network = Connection<TypeConfig>;

    async fn new_client(&mut self, target: NodeId, _node: &()) -> Self::Network {
        Connection {
            router: self.clone(),
            target,
        }
    }
}

impl<C: RaftTypeConfig> P2pNetwork<C> for Router<C>
where
    C: RaftTypeConfig<Responder = OneshotResponder<C>>,
    C::SnapshotData: Debug,
    C::R: Debug,
{
    fn send(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest,
    ) -> impl Future<Output = anyhow::Result<()>> {
        async move {
            let r = self.lock().await;
            let tgt = r.targets.get(&target).unwrap();
            tgt.handle_p2p_request(source, req).await?;
            Ok(())
        }
    }
}
impl RaftNetworkV2<TypeConfig> for Connection<TypeConfig> {
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<TypeConfig>, RPCError<TypeConfig>> {
        match self
            .router
            .rpc_request(self.target, RaftRequest::Append(req).into())
            .await
        {
            Ok(resp) => Ok(resp.unwrap_raft().unwrap_append()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(e.into())
            }
        }
    }

    /// A real application should replace this method with customized implementation.
    async fn full_snapshot(
        &mut self,
        vote: Vote<TypeConfig>,
        snapshot: Snapshot<TypeConfig>,
        _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<TypeConfig>, StreamingError<TypeConfig>> {
        match self
            .router
            .rpc_request(self.target, RaftRequest::Snapshot { vote, snapshot }.into())
            .await
        {
            Ok(resp) => Ok(resp.unwrap_raft().unwrap_snapshot()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(e.into())
            }
        }
    }

    async fn vote(
        &mut self,
        req: VoteRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig>> {
        match self
            .router
            .rpc_request(self.target, RaftRequest::Vote(req).into())
            .await
        {
            Ok(resp) => Ok(resp.unwrap_raft().unwrap_vote()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(e.into())
            }
        }
    }
}
