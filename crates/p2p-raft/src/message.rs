use openraft::{
    error::{ClientWriteError, Infallible, RaftError},
    raft::*,
    SnapshotMeta,
};

use crate::TypeConf;

#[derive(Debug, derive_more::From, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum RpcRequest<C: TypeConf> {
    Raft(RaftRequest<C>),
    P2p(P2pRequest<C>),
}

impl<C: TypeConf> Clone for RpcRequest<C> {
    fn clone(&self) -> Self {
        match self {
            Self::Raft(r) => Self::Raft(r.clone()),
            Self::P2p(p) => Self::P2p(p.clone()),
        }
    }
}

#[derive(Debug, derive_more::From, derive_more::Unwrap, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum RpcResponse<C: RaftTypeConfig> {
    Raft(RaftResponse<C>),
    P2p(P2pResponse<C>),
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum P2pRequest<C: RaftTypeConfig> {
    Propose(C::D),
    Join,
    Leave,
}

#[derive(Debug, derive_more::Unwrap, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum P2pResponse<C: RaftTypeConfig> {
    Ok,
    RaftError(RaftError<C, ClientWriteError<C>>),
    P2pError(P2pError),
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub enum P2pError {
    NotVoter,
}

#[derive(Debug, derive_more::From, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum RaftRequest<C: RaftTypeConfig> {
    Append(AppendEntriesRequest<C>),
    Snapshot {
        vote: C::Vote,
        snapshot_meta: SnapshotMeta<C>,
        snapshot_data: C::SnapshotData,
    },
    Vote(VoteRequest<C>),
}

impl<C: TypeConf> Clone for RaftRequest<C> {
    fn clone(&self) -> Self {
        match self {
            Self::Append(a) => Self::Append(a.clone()),
            Self::Snapshot {
                vote,
                snapshot_meta,
                snapshot_data,
            } => Self::Snapshot {
                vote: vote.clone(),
                snapshot_meta: snapshot_meta.clone(),
                snapshot_data: snapshot_data.clone(),
            },
            Self::Vote(v) => Self::Vote(v.clone()),
        }
    }
}

#[derive(Debug, derive_more::From, derive_more::Unwrap, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum RaftResponse<C: RaftTypeConfig> {
    Append(AppendEntriesResponse<C>),
    Snapshot(SnapshotResponse<C>),
    Vote(VoteResponse<C>),

    // XXX: hmm
    Error(Infallible),
}
