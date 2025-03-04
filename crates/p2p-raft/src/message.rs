use openraft::{
    error::{ClientWriteError, Infallible, RaftError},
    raft::*,
    Snapshot,
};

use crate::TypeConf;

#[derive(Debug, derive_more::From)]
pub enum RpcRequest<C: TypeConf>
where
    C::D: std::fmt::Debug,
    C::SnapshotData: std::fmt::Debug,
{
    Raft(RaftRequest<C>),
    P2p(P2pRequest<C>),
}

#[derive(Debug, derive_more::From, derive_more::Unwrap)]
pub enum RpcResponse<C: TypeConf> {
    Ok,
    Raft(RaftResponse<C>),
    P2p(P2pResponse<C>),
}

#[derive(Debug)]
pub enum P2pRequest<C: TypeConf> {
    Propose(C::D),
    Join,
    Leave,
}

#[derive(Debug, derive_more::Unwrap)]
pub enum P2pResponse<C: TypeConf> {
    Ok,
    RaftError(RaftError<C, ClientWriteError<C>>),
    P2pError(P2pError),
}

#[derive(Debug)]
pub enum P2pError {
    NotVoter,
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

    // XXX: hmm
    Error(Infallible),
}
