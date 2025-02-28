use std::{collections::BTreeSet, fmt::Debug, sync::Arc};

use network_impl::{NodeId, TypeConfig};
use openraft::{
    Raft, RaftTypeConfig,
    error::{InitializeError, RaftError},
};
use tokio::sync::Mutex;

use crate::peer_tracker::PeerTracker;

#[derive(Clone, derive_more::From, derive_more::Deref)]
pub struct Dinghy<C: RaftTypeConfig> {
    #[deref]
    pub raft: Raft<C>,
    pub tracker: Arc<Mutex<PeerTracker<C>>>,
}

impl<C: RaftTypeConfig> Dinghy<C> {
    pub fn new(raft: Raft<C>) -> Self {
        Self {
            raft,
            tracker: PeerTracker::new(),
        }
    }

    pub async fn initialize(
        &self,
        ids: impl IntoIterator<Item = C::NodeId>,
    ) -> Result<(), RaftError<C, InitializeError<C>>>
    where
        BTreeSet<<C as RaftTypeConfig>::NodeId>: openraft::membership::IntoNodes<
                <C as RaftTypeConfig>::NodeId,
                <C as RaftTypeConfig>::Node,
            >,
    {
        let ids: BTreeSet<C::NodeId> = ids.into_iter().collect::<BTreeSet<_>>();
        match self.raft.initialize(ids).await {
            Ok(_) => Ok(()),
            // this error is ok, it means we got some network messages already
            Err(RaftError::APIError(InitializeError::NotAllowed(_))) => Ok(()),
            e => e,
        }
    }
}
