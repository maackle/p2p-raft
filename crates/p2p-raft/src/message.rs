use openraft::{
    error::{ClientWriteError, Infallible, RaftError},
    raft::*,
    SnapshotMeta,
};

use crate::TypeCfg;

/// An RPC request sent from one node to another.
#[derive(Debug, derive_more::From, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum Request<C: TypeCfg> {
    /// A raft protocol RPC.
    Raft(RaftRequest<C>),

    /// The extra layer on top of standard Raft provided by this crate.
    /// These calls must always be handled by the current leader,
    /// they will fail if handled by a non-leader.
    P2p(P2pRequest<C>),
}

impl<C: TypeCfg> Clone for Request<C> {
    fn clone(&self) -> Self {
        match self {
            Self::Raft(r) => Self::Raft(r.clone()),
            Self::P2p(p) => Self::P2p(p.clone()),
        }
    }
}

/// An RPC response to [`Request`].
#[derive(Debug, derive_more::From, derive_more::Unwrap, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum Response<C: RaftTypeConfig> {
    Raft(RaftResponse<C>),
    P2p(P2pResponse<C>),
}

/// Request that the leader modify the log on your behalf.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum P2pRequest<C: RaftTypeConfig> {
    /// Propose a new value to the raft cluster.
    Propose(C::D),

    /// Join the raft cluster.
    Join,

    /// Leave the raft cluster.
    Leave,
}

/// Response to a [`P2pRequest`].
#[derive(
    Clone, Debug, PartialEq, Eq, derive_more::Unwrap, serde::Serialize, serde::Deserialize,
)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum P2pResponse<C: RaftTypeConfig> {
    Ok,
    RaftError(RaftError<C, ClientWriteError<C>>),
    P2pError(P2pError),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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

impl<C: TypeCfg> Clone for RaftRequest<C> {
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
