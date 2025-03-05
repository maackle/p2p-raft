use std::future::Future;

use p2p_raft_memstore::NodeId;
use p2p_raft_memstore::TypeConfig;
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
use crate::TypeConf;

use super::router::RouterNode;
use super::Router;

pub struct Connection<C: TypeConf>
where
    C::SnapshotData: std::fmt::Debug,
    C::SnapshotData: serde::Serialize + serde::de::DeserializeOwned,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
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

impl<C: TypeConf> P2pNetwork<C> for Router<C>
where
    C::SnapshotData: std::fmt::Debug,
    C::SnapshotData: serde::Serialize + serde::de::DeserializeOwned,
    C::D: std::fmt::Debug,
    C::R: std::fmt::Debug,
{
    fn send(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest<C>,
    ) -> impl Future<Output = P2pResponse<C>> {
        async move {
            // dbg!(&source);
            let tgt = self.lock().targets.get(&target).unwrap().clone();
            tgt.handle_p2p_request(source.clone(), req)
                .await
                .expect("fatal error")
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
                    snapshot_data: *snapshot.snapshot,
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
