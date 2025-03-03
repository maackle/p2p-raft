use openraft::{RaftTypeConfig, Snapshot, raft::*};

#[derive(Debug, derive_more::From)]
pub enum RpcRequest<C: RaftTypeConfig>
where
    C::SnapshotData: std::fmt::Debug,
{
    Raft(RaftRequest<C>),
    P2p(P2pRequest),
}

#[derive(Debug, derive_more::From, derive_more::Unwrap)]
pub enum RpcResponse<C: RaftTypeConfig> {
    Ok,
    Raft(RaftResponse<C>),
}

#[derive(Debug)]
pub enum P2pRequest {
    Join,
    Leave,
}

#[derive(Debug, derive_more::From)]
pub enum RaftRequest<C: RaftTypeConfig>
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
pub enum RaftResponse<C: RaftTypeConfig> {
    Append(AppendEntriesResponse<C>),
    Snapshot(SnapshotResponse<C>),
    Vote(VoteResponse<C>),
}
