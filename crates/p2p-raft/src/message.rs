use openraft::{raft::*, Snapshot};

use crate::TypeConf;

#[derive(Debug, derive_more::From)]
pub enum RpcRequest<C: TypeConf>
where
    C::SnapshotData: std::fmt::Debug,
{
    Raft(RaftRequest<C>),
    P2p(P2pRequest),
}

#[derive(Debug, derive_more::From, derive_more::Unwrap)]
pub enum RpcResponse<C: TypeConf> {
    Ok,
    Raft(RaftResponse<C>),
}

#[derive(Debug)]
pub enum P2pRequest {
    Join,
    Leave,
}

#[derive(Debug, derive_more::From)]
pub enum RaftRequest<C: TypeConf>
where
    C::SnapshotData: std::fmt::Debug,
{
    Append(AppendEntriesRequest<C>),
    Snapshot {
        vote: C::Vote,
        snapshot: Snapshot<C>,
    },
    Vote(VoteRequest<C>),
}

#[derive(derive_more::From, Debug, derive_more::Unwrap)]
pub enum RaftResponse<C: TypeConf> {
    Append(AppendEntriesResponse<C>),
    Snapshot(SnapshotResponse<C>),
    Vote(VoteResponse<C>),
}
