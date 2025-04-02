use openraft::{error::RaftError, RaftTypeConfig};

use crate::message::ForwardToLeader;

#[derive(
    Clone,
    Debug,
    PartialEq,
    Eq,
    derive_more::Display,
    derive_more::Error,
    serde::Serialize,
    serde::Deserialize,
)]
pub enum P2pRaftError<C: RaftTypeConfig, F = anyhow::Error> {
    /// Callee rejected request because caller is not a voter.
    /// Call should work if this node successfully joins the cluster.
    Rejected,

    /// Attempted to call a non-leader.
    /// The call should work when a leader is elected.
    NotLeader(ForwardToLeader<C>),

    /// Fatal error, something unrecoverable went wrong.
    /// Retrying is not likely to succeed.
    Fatal(F),
}

impl<C: RaftTypeConfig> From<anyhow::Error> for P2pRaftError<C, anyhow::Error> {
    fn from(e: anyhow::Error) -> Self {
        Self::Fatal(e)
    }
}

impl<C: RaftTypeConfig> From<String> for P2pRaftError<C, String> {
    fn from(e: String) -> Self {
        Self::Fatal(e)
    }
}

impl<C: RaftTypeConfig> From<P2pRaftError<C, anyhow::Error>> for P2pRaftError<C, String> {
    fn from(e: P2pRaftError<C, anyhow::Error>) -> Self {
        match e {
            P2pRaftError::Rejected => Self::Rejected,
            P2pRaftError::NotLeader(fwd) => Self::NotLeader(fwd),
            P2pRaftError::Fatal(e) => Self::Fatal(e.to_string()),
        }
    }
}

impl<C: RaftTypeConfig> From<P2pRaftError<C, String>> for P2pRaftError<C, anyhow::Error> {
    fn from(e: P2pRaftError<C, String>) -> Self {
        match e {
            P2pRaftError::Rejected => Self::Rejected,
            P2pRaftError::NotLeader(fwd) => Self::NotLeader(fwd),
            P2pRaftError::Fatal(e) => Self::Fatal(anyhow::anyhow!(e)),
        }
    }
}

impl<C: RaftTypeConfig, E, F> From<RaftError<C, E>> for P2pRaftError<C, F>
where
    E: std::fmt::Debug
        + std::error::Error
        + openraft::TryAsRef<openraft::error::ForwardToLeader<C>>
        + Send
        + Sync
        + 'static,
    F: From<RaftError<C, E>>,
{
    fn from(e: RaftError<C, E>) -> Self {
        if let Some(fwd) = e.forward_to_leader() {
            Self::NotLeader(crate::message::raft_forward_to_leader(fwd))
        } else {
            Self::Fatal(F::from(e))
        }
    }
}

pub type P2pRaftResult<T, C> = Result<T, P2pRaftError<C>>;

pub trait P2pRaftResultExt<T, C: RaftTypeConfig>: Sized {
    fn result(self) -> Result<T, P2pRaftError<C>>;

    fn nonfatal(self) -> P2pRaftResult<(), C> {
        match self.result() {
            Ok(_) => Ok(()),
            Err(P2pRaftError::NotLeader(_) | P2pRaftError::Rejected) => Ok(()),
            r @ Err(P2pRaftError::Fatal(_)) => r.map(|_| ()),
        }
    }
}

impl<T, C: RaftTypeConfig> P2pRaftResultExt<T, C> for P2pRaftResult<T, C> {
    fn result(self) -> Result<T, P2pRaftError<C>> {
        self
    }
}
