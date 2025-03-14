use std::future::Future;

use openraft::error::RPCError;
use openraft::error::ReplicationClosed;
use openraft::error::StreamingError;
use openraft::error::Unreachable;
use openraft::network::v2::RaftNetworkV2;
use openraft::network::RPCOption;
use openraft::raft::AppendEntriesRequest;
use openraft::raft::AppendEntriesResponse;
use openraft::raft::SnapshotResponse;
use openraft::raft::VoteRequest;
use openraft::raft::VoteResponse;
use openraft::OptionalSend;
use openraft::RaftNetworkFactory;
use openraft::Snapshot;
use openraft::Vote;

use crate::message::P2pRequest;
use crate::message::P2pResponse;
use crate::message::RaftRequest;
use crate::network::P2pNetwork;

use super::router::RouterNode;
use super::NodeId;
use super::TypeConfig;

pub struct Connection {
    router: RouterNode,
    target: NodeId,
}

impl RaftNetworkFactory<TypeConfig> for RouterNode {
    type Network = Connection;

    async fn new_client(&mut self, target: NodeId, _node: &()) -> Self::Network {
        Connection {
            router: self.clone(),
            target,
        }
    }
}

impl P2pNetwork<TypeConfig> for RouterNode {
    fn local_node_id(&self) -> NodeId {
        self.source
    }

    async fn send_p2p(
        &self,
        target: NodeId,
        req: P2pRequest<TypeConfig>,
    ) -> anyhow::Result<P2pResponse<TypeConfig>> {
        let tgt = self.router.lock().targets.get(&target).unwrap().clone();
        tgt.handle_p2p_request(self.local_node_id(), req)
            .await
            .map_err(Into::into)
    }
}
impl RaftNetworkV2<TypeConfig> for Connection {
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
                Err(RPCError::Unreachable(Unreachable::new(&e)))
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
            .rpc_request(
                self.target,
                RaftRequest::Snapshot {
                    vote,
                    snapshot_meta: snapshot.meta,
                    snapshot_data: snapshot.snapshot,
                }
                .into(),
            )
            .await
        {
            Ok(resp) => Ok(resp.unwrap_raft().unwrap_snapshot()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(StreamingError::Unreachable(Unreachable::new(&e)))
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
                Err(RPCError::Unreachable(Unreachable::new(&e)))
            }
        }
    }
}
