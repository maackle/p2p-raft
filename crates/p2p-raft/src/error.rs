use derive_more::derive::{Display, Error};
use openraft::{error::RaftError, RaftTypeConfig};

use crate::message::ForwardToLeader;

#[derive(Clone, Debug, Display, Error, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
// #[serde(from = "P2pRaftErrorWire<C>", into = "P2pRaftErrorWire<C>")]
pub enum PError<C: RaftTypeConfig, F = anyhow::Error> {
    /// Callee rejected request because caller is not a voter
    NotVoter,

    /// Attempted to call a non-leader
    NotLeader(ForwardToLeader<C>),

    Fatal(F),
}

impl<C: RaftTypeConfig> From<anyhow::Error> for PError<C, anyhow::Error> {
    fn from(e: anyhow::Error) -> Self {
        Self::Fatal(e)
    }
}

impl<C: RaftTypeConfig> From<String> for PError<C, String> {
    fn from(e: String) -> Self {
        Self::Fatal(e)
    }
}

// #[derive(Debug, Display, From, serde::Serialize, serde::Deserialize)]
// pub enum P2pRaftErrorWire<C: RaftTypeConfig> {
//     NotLeader(ForwardToLeader<C>),

//     Fatal(String),
// }

impl<C: RaftTypeConfig> From<PError<C, anyhow::Error>> for PError<C, String> {
    fn from(e: PError<C, anyhow::Error>) -> Self {
        match e {
            PError::NotVoter => Self::NotVoter,
            PError::NotLeader(fwd) => Self::NotLeader(fwd),
            PError::Fatal(e) => Self::Fatal(e.to_string()),
        }
    }
}

impl<C: RaftTypeConfig> From<PError<C, String>> for PError<C, anyhow::Error> {
    fn from(e: PError<C, String>) -> Self {
        match e {
            PError::NotVoter => Self::NotVoter,
            PError::NotLeader(fwd) => Self::NotLeader(fwd),
            PError::Fatal(e) => Self::Fatal(anyhow::anyhow!(e)),
        }
    }
}

impl<C: RaftTypeConfig, E, F> From<RaftError<C, E>> for PError<C, F>
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

pub type PResult<T, C> = Result<T, PError<C>>;

pub trait PResultExt<T, C: RaftTypeConfig>: Sized {
    fn result(self) -> Result<T, PError<C>>;

    fn nonfatal(self) -> PResult<(), C> {
        match self.result() {
            Ok(_) => Ok(()),
            Err(PError::NotLeader(_) | PError::NotVoter) => Ok(()),
            r @ Err(PError::Fatal(_)) => r.map(|_| ()),
        }
    }
}

impl<T, C: RaftTypeConfig> PResultExt<T, C> for PResult<T, C> {
    fn result(self) -> Result<T, PError<C>> {
        self
    }
}
