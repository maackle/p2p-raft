use std::fmt::Debug;
use std::sync::Arc;
use std::sync::Mutex;

use openraft::alias::SnapshotDataOf;
use openraft::storage::RaftStateMachine;
use openraft::*;

use crate::StateMachineData;

#[derive(Debug)]
pub struct StoredSnapshot<C: RaftTypeConfig> {
    pub meta: SnapshotMeta<C>,

    /// The data of the state machine at the time of this snapshot.
    pub data: C::SnapshotData,
}

/// Awkward. Need this newtype to implement traits generically.
#[derive(Clone, derive_more::Deref, derive_more::From)]
pub struct ArcStateMachineStore<C: RaftTypeConfig>(Arc<StateMachineStore<C>>);

/// Defines a state machine for the Raft cluster. This state machine represents a copy of the
/// data for this node. Additionally, it is responsible for storing the last snapshot of the data.
pub struct StateMachineStore<C: RaftTypeConfig> {
    /// The Raft state machine.
    pub state_machine: Mutex<StateMachineData<C>>,

    snapshot_idx: Mutex<u64>,

    /// The last received snapshot.
    current_snapshot: Mutex<Option<StoredSnapshot<C>>>,
}

impl<C: RaftTypeConfig> Default for StateMachineStore<C> {
    fn default() -> Self {
        Self {
            state_machine: Mutex::new(StateMachineData::default()),
            snapshot_idx: Mutex::new(0),
            current_snapshot: Mutex::new(None),
        }
    }
}

impl<C: RaftTypeConfig> RaftSnapshotBuilder<C> for ArcStateMachineStore<C>
where
    C: RaftTypeConfig<Entry = openraft::Entry<C>, SnapshotData = StateMachineData<C>>,
    C::SnapshotData: Clone,
    StateMachineData<C>: Clone,
{
    #[tracing::instrument(level = "trace", skip(self))]
    async fn build_snapshot(&mut self) -> Result<Snapshot<C>, StorageError<C>> {
        let data;
        let last_applied_log;
        let last_membership;

        {
            // Serialize the data of the state machine.
            let state_machine = self.state_machine.lock().unwrap();

            last_applied_log = state_machine.last_applied.clone();
            last_membership = state_machine.last_membership.clone();
            data = state_machine.clone();
        }

        let snapshot_idx = {
            let mut l = self.snapshot_idx.lock().unwrap();
            *l += 1;
            *l
        };

        let snapshot_id = if let Some(last) = &last_applied_log {
            format!(
                "{}-{}-{}",
                last.committed_leader_id(),
                last.index(),
                snapshot_idx
            )
        } else {
            format!("--{}", snapshot_idx)
        };

        let meta = SnapshotMeta {
            last_log_id: last_applied_log.clone(),
            last_membership,
            snapshot_id,
        };

        let snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: data.clone(),
        };

        {
            let mut current_snapshot = self.current_snapshot.lock().unwrap();
            *current_snapshot = Some(snapshot);
        }

        Ok(Snapshot {
            meta,
            snapshot: Box::new(data),
        })
    }
}

impl<C: RaftTypeConfig> RaftStateMachine<C> for ArcStateMachineStore<C>
where
    C: RaftTypeConfig<Entry = openraft::Entry<C>, SnapshotData = StateMachineData<C>, R = ()>,
    C::SnapshotData: Clone,
    StateMachineData<C>: Clone,
{
    type SnapshotBuilder = Self;

    async fn applied_state(
        &mut self,
    ) -> Result<(Option<LogId<C>>, StoredMembership<C>), StorageError<C>> {
        let state_machine = self.state_machine.lock().unwrap();
        Ok((
            state_machine.last_applied.clone(),
            state_machine.last_membership.clone(),
        ))
    }

    #[tracing::instrument(level = "trace", skip(self, entries))]
    async fn apply<I>(&mut self, entries: I) -> Result<Vec<C::R>, StorageError<C>>
    where
        I: IntoIterator<Item = Entry<C>>,
    {
        let mut res = Vec::new(); //No `with_capacity`; do not know `len` of iterator

        let mut sm = self.state_machine.lock().unwrap();

        for entry in entries {
            tracing::debug!(%entry.log_id, "replicate to sm");

            sm.last_applied = Some(entry.log_id);

            if let EntryPayload::Normal(d) = entry.payload {
                sm.data.push(d);
            }

            res.push(())
        }
        Ok(res)
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<SnapshotDataOf<C>>, StorageError<C>> {
        Ok(Box::default())
    }

    #[tracing::instrument(level = "trace", skip(self, snapshot))]
    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<C>,
        snapshot: Box<SnapshotDataOf<C>>,
    ) -> Result<(), StorageError<C>> {
        tracing::info!("install snapshot");

        let new_snapshot = StoredSnapshot {
            meta: meta.clone(),
            data: (*snapshot).clone(),
        };

        // Update the state machine.
        {
            let updated_state_machine: StateMachineData<C> = new_snapshot.data.clone();
            let mut state_machine = self.state_machine.lock().unwrap();
            *state_machine = updated_state_machine;
        }

        // Update current snapshot.
        let mut current_snapshot = self.current_snapshot.lock().unwrap();
        *current_snapshot = Some(new_snapshot);
        Ok(())
    }

    #[tracing::instrument(level = "trace", skip(self))]
    async fn get_current_snapshot(&mut self) -> Result<Option<Snapshot<C>>, StorageError<C>> {
        match &*self.current_snapshot.lock().unwrap() {
            Some(snapshot) => {
                let data = snapshot.data.clone();
                Ok(Some(Snapshot {
                    meta: snapshot.meta.clone(),
                    snapshot: Box::new(data),
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        self.clone()
    }
}
