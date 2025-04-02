use derive_more::derive::{Display, Error, From};
use openraft::RaftTypeConfig;

use crate::message::ForwardToLeader;

#[derive(Debug, Display, Error, From)]
pub enum P2pRaftError<C: RaftTypeConfig> {
    NotLeader(ForwardToLeader<C>),

    Fatal(anyhow::Error),
}

pub type PResult<T, C> = Result<T, P2pRaftError<C>>;

pub trait PResultExt<T, C: RaftTypeConfig>: Sized {
    fn result(self) -> Result<T, P2pRaftError<C>>;

    fn nonfatal(self) -> PResult<(), C> {
        match self.result() {
            Ok(_) => Ok(()),
            Err(P2pRaftError::NotLeader(_)) => Ok(()),
            r @ Err(P2pRaftError::Fatal(_)) => r.map(|_| ()),
        }
    }
}

impl<T, C: RaftTypeConfig> PResultExt<T, C> for PResult<T, C> {
    fn result(self) -> Result<T, P2pRaftError<C>> {
        self
    }
}
