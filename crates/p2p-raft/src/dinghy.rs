use std::{borrow::Borrow, collections::BTreeSet, fmt::Debug, sync::Arc};

use memstore::StateMachineStore;
use openraft::{
    Entry, EntryPayload, Raft, RaftTypeConfig, StorageError,
    alias::ResponderReceiverOf,
    error::{InitializeError, RaftError},
    raft::ClientWriteResult,
};
use tokio::sync::Mutex;

use crate::peer_tracker::PeerTracker;

#[derive(Clone, derive_more::From, derive_more::Deref)]
pub struct Dinghy<C: RaftTypeConfig> {
    #[deref]
    pub raft: Raft<C>,
    pub id: C::NodeId,
    pub store: memstore::LogStore<C>,
    pub tracker: Arc<Mutex<PeerTracker<C>>>,
}

impl<C: RaftTypeConfig> Dinghy<C> {
    pub fn new(id: C::NodeId, raft: Raft<C>, store: memstore::LogStore<C>) -> Self {
        Self {
            id,
            raft,
            store,
            tracker: PeerTracker::new(),
        }
    }

    // pub fn id(&self) -> C::NodeId
    // where
    //     C: RaftTypeConfig<AsyncRuntime = openraft::TokioRuntime>,
    // {
    //     self.raft.metrics().borrow().id.clone()
    // }

    // pub async fn is_leader(&self) -> bool {
    //     #[allow(deprecated)]
    //     self.raft.is_leader().await.is_ok()
    // }

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

    pub async fn read_log_data(&self) -> anyhow::Result<Vec<C::D>>
    where
        C: RaftTypeConfig<Entry = Entry<C>>,
        C::Entry: Clone,
    {
        use openraft::RaftLogReader;
        use openraft::storage::RaftLogStorage;

        Ok(self
            .store
            .clone()
            .get_log_reader()
            .await
            .try_get_log_entries(..)
            .await?
            .into_iter()
            .filter_map(|e: Entry<C>| match e.payload {
                EntryPayload::Normal(n) => Some(n),
                _ => None,
            })
            .collect::<Vec<_>>())
    }

    pub async fn write_linearizable<E>(&self, data: C::D) -> anyhow::Result<()>
    where
        ResponderReceiverOf<C>: Future<Output = Result<ClientWriteResult<C>, E>>,
        E: std::error::Error + openraft::OptionalSend,
    {
        self.ensure_linearizable().await?;
        self.raft.client_write(data).await?;
        Ok(())
    }
}
