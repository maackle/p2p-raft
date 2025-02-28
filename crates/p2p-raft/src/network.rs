use std::future::Future;
use std::time::Duration;

use openraft::OptionalSend;
use openraft::RPCTypes;
use openraft::RaftNetworkFactory;
use openraft::RaftTypeConfig;
use openraft::Snapshot;
use openraft::Vote;
use openraft::error::ReplicationClosed;
use openraft::error::Timeout;
use openraft::network::RPCOption;
use openraft::network::v2::RaftNetworkV2;
use openraft::raft::AppendEntriesRequest;
use openraft::raft::AppendEntriesResponse;
use openraft::raft::SnapshotResponse;
use openraft::raft::VoteRequest;
use openraft::raft::VoteResponse;

use crate::router::RouterNode;
use network_impl::NodeId;
use network_impl::TypeConfig;
use network_impl::typ;

pub struct Connection {
    router: RouterNode<TypeConfig>,
    target: NodeId,
}

impl RaftNetworkFactory<TypeConfig> for RouterNode<TypeConfig> {
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
        req: typ::AppendEntriesRequest,
        _option: RPCOption,
    ) -> Result<typ::AppendEntriesResponse, typ::RPCError> {
        match self
            .router
            .raft_request(self.target, RaftRequest::Append(req))
            .await
        {
            Ok(resp) => Ok(resp.unwrap_append()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(e.into())
            }
        }
    }

    /// A real application should replace this method with customized implementation.
    async fn full_snapshot(
        &mut self,
        vote: typ::Vote,
        snapshot: typ::Snapshot,
        _cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<typ::SnapshotResponse, typ::StreamingError> {
        match self
            .router
            .raft_request(self.target, RaftRequest::Snapshot { vote, snapshot })
            .await
        {
            Ok(resp) => Ok(resp.unwrap_snapshot()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(e.into())
            }
        }
    }

    async fn vote(
        &mut self,
        req: typ::VoteRequest,
        _option: RPCOption,
    ) -> Result<typ::VoteResponse, typ::RPCError> {
        match self
            .router
            .raft_request(self.target, RaftRequest::Vote(req))
            .await
        {
            Ok(resp) => Ok(resp.unwrap_vote()),
            Err(e) => {
                tracing::error!("{e:?}");
                Err(e.into())
            }
        }
    }
}

#[derive(derive_more::From)]
pub enum RaftRequest<C: RaftTypeConfig> {
    Append(AppendEntriesRequest<C>),
    Snapshot {
        vote: C::Vote,
        snapshot: Snapshot<C>,
    },
    Vote(VoteRequest<C>),
}

#[derive(derive_more::From, derive_more::Unwrap)]
pub enum RaftResponse<C: RaftTypeConfig> {
    Append(AppendEntriesResponse<C>),
    Snapshot(SnapshotResponse<C>),
    Vote(VoteResponse<C>),
}
