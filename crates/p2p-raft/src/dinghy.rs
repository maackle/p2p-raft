use std::{borrow::Borrow, collections::BTreeSet, fmt::Debug, sync::Arc};

use futures::FutureExt;
use memstore::StateMachineStore;
use openraft::{
    Entry, EntryPayload, Raft, RaftTypeConfig, SnapshotMeta, StorageError,
    alias::ResponderReceiverOf,
    error::{InitializeError, RaftError},
    raft::ClientWriteResult,
    storage::RaftStateMachine,
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

impl Dinghy<memstore::TypeConfig> {
    /// NOTE: only run this as leader!
    /// XXX: really this is just a workaround for when it's not feasible to implement
    ///      merging in the state machine, when that logic needs to be in the front end
    ///      and the merged snapshot is forced by the leader.
    #[deprecated = "this does not work!"]
    pub async fn replace_snapshot(&self, data: Vec<memstore::Request>) {
        use openraft::storage::{RaftSnapshotBuilder, RaftStateMachine};

        let smd = {
            let mut sm = self
                .raft
                .with_state_machine(|s: &mut Arc<StateMachineStore<memstore::TypeConfig>>| {
                    async move { s.state_machine.lock().unwrap().clone() }.boxed()
                })
                .await
                .unwrap()
                .unwrap();

            sm.data = data;
            // sm.last_applied.map(|mut l| {
            //     l.index += 1;
            //     l
            // });
            sm
        };

        let snapshot = Box::new(smd.clone());

        let snapshot_id = self
            .raft
            .with_raft_state(|s| s.snapshot_meta.snapshot_id.clone())
            .await
            .unwrap();

        let meta = SnapshotMeta {
            last_log_id: smd.last_applied,
            last_membership: smd.last_membership,
            snapshot_id,
            // snapshot_id: nanoid::nanoid!(),
        };

        self.with_state_machine(
            move |s: &mut Arc<StateMachineStore<memstore::TypeConfig>>| {
                async move {
                    s.clone()
                        .install_snapshot(&meta.clone(), snapshot)
                        .await
                        .unwrap();
                    // s.build_snapshot().await.unwrap();
                }
                .boxed()
            },
        )
        .await
        .unwrap()
        .unwrap();

        let trigger = self.raft.trigger();
        trigger
            .purge_log(smd.last_applied.map(|l| l.index).unwrap_or_default())
            .await
            .unwrap();
        trigger.snapshot().await.unwrap();
        trigger.heartbeat().await.unwrap();

        // raft.install_full_snapshot(Vote::new(term, raft.id), snapshot)
        //     .await
        //     .unwrap();
    }
}
