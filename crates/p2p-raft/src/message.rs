use openraft::{alias::LogIdOf, raft::*, SnapshotMeta};

use crate::{error::P2pRaftError, TypeCfg};

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
pub enum Response<C: TypeCfg> {
    Raft(RaftResponse<C>),
    P2p(P2pResponse<C>),
}

impl<C: TypeCfg> Response<C> {
    pub fn is_ok(&self) -> bool {
        match self {
            Self::Raft(r) => r.is_ok(),
            Self::P2p(p) => p.is_ok(),
        }
    }
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
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub enum P2pResponse<C: TypeCfg> {
    Ok,
    Committed(Committed<C>),
    Error(P2pRaftError<C, String>),
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::SnapshotData: serde::de::DeserializeOwned"
))]
pub struct Committed<C: TypeCfg> {
    pub log_id: LogIdOf<C>,
    pub prev_op_log_id: Option<LogIdOf<C>>,
}

pub type ForwardToLeader<C> = Option<(<C as RaftTypeConfig>::NodeId, <C as RaftTypeConfig>::Node)>;

pub(crate) fn raft_forward_to_leader<C: RaftTypeConfig>(
    rf: &openraft::error::ForwardToLeader<C>,
) -> ForwardToLeader<C> {
    Some((rf.leader_id.clone()?, rf.leader_node.clone()?))
}

impl<C: TypeCfg> P2pResponse<C> {
    pub fn forward_to_leader(&self) -> Option<ForwardToLeader<C>> {
        match self {
            Self::Error(P2pRaftError::NotLeader(e)) => Some(e.clone()),
            _ => None,
        }
    }

    pub fn to_error(self) -> anyhow::Result<()> {
        match self {
            Self::Ok => Ok(()),
            Self::Committed { .. } => Ok(()),
            Self::Error(e) => Err(anyhow::anyhow!("{e:?}")),
        }
    }

    pub fn is_ok(&self) -> bool {
        match self {
            P2pResponse::Ok => true,
            P2pResponse::Committed { .. } => true,
            P2pResponse::Error(_) => false,
        }
    }
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
}

impl<C: TypeCfg> RaftResponse<C> {
    pub fn is_ok(&self) -> bool {
        match self {
            Self::Append(r) => r.is_success(),
            Self::Snapshot(_) => true,
            Self::Vote(r) => r.vote_granted,
        }
    }
}
