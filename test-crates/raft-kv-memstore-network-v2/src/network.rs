use std::future::Future;

use openraft::error::ReplicationClosed;
use openraft::network::v2::RaftNetworkV2;
use openraft::network::RPCOption;
use openraft::OptionalSend;
use openraft::RaftNetworkFactory;

use crate::app::RaftRequest;
use crate::app::RpcRequest;
use crate::router::Router;
use crate::router::RouterNode;
use crate::typ::*;
use crate::NodeId;
use crate::TypeConfig;

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

impl RaftNetworkV2<TypeConfig> for Connection {
    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse, RPCError> {
        let resp = self
            .router
            .send(self.target, RpcRequest::Raft(RaftRequest::Append(req)))
            .await?;
        Ok(resp.unwrap_raft().unwrap_append())
    }

    /// A real application should replace this method with customized implementation.
    async fn full_snapshot(
        &mut self,
        vote: Vote,
        snapshot: Snapshot,
        _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse, StreamingError> {
        let resp = self
            .router
            .send(
                self.target,
                RpcRequest::Raft(RaftRequest::Snapshot { vote, snapshot }),
            )
            .await?;
        Ok(resp.unwrap_raft().unwrap_snapshot())
    }

    async fn vote(
        &mut self,
        req: VoteRequest,
        _option: RPCOption,
    ) -> Result<VoteResponse, RPCError> {
        let resp = self
            .router
            .send(self.target, RpcRequest::Raft(RaftRequest::Vote(req)))
            .await?;
        Ok(resp.unwrap_raft().unwrap_vote())
    }
}
