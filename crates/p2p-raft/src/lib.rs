use std::collections::BTreeSet;

use network_impl::{NodeId, TypeConfig, typ::Raft};
use openraft::error::{InitializeError, RaftError};

#[derive(Clone, derive_more::From, derive_more::Deref)]
pub struct Dinghy(Raft);

impl Dinghy {
    pub async fn initialize(
        &self,
        ids: impl IntoIterator<Item = NodeId>,
    ) -> Result<(), RaftError<TypeConfig, InitializeError<TypeConfig>>> {
        match self
            .0
            .initialize(ids.into_iter().collect::<BTreeSet<_>>())
            .await
        {
            Ok(_) => Ok(()),
            // this error is ok, it means we got some network messages already
            Err(RaftError::APIError(InitializeError::NotAllowed(_))) => Ok(()),
            e => e,
        }
    }
}
