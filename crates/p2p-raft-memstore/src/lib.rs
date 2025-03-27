//! Provides basic in-memory storage layer implementations, for simple use cases or as a starting point.
//!
//! adapted from https://github.com/databendlabs/openraft/blob/cc2b37ad8b10c1871fb4dbc9b9422234f0b222ec/stores/memstore/src/lib.rs

mod log_store;
mod state_machine;

pub use log_store::LogStore;
use openraft::RaftTypeConfig;
pub use state_machine::{ArcStateMachineStore, StateMachineStore};

pub type SnapshotData<C> = Vec<<C as RaftTypeConfig>::D>;

/// Data contained in the Raft state machine.
///
/// Note that we are using `serde` to serialize the
/// `data`, which has a implementation to be serialized. Note that for this test we set both the key
/// and value as String, but you could set any type of value that has the serialization impl.
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct StateMachineData<C: RaftTypeConfig>
where
    C: RaftTypeConfig<SnapshotData = SnapshotData<C>>,
    C::D: Clone + std::fmt::Debug,
{
    pub last_applied: Option<openraft::LogId<C>>,

    pub last_membership: openraft::StoredMembership<C>,

    /// Application data, just a list of requests.
    pub data: SnapshotData<C>,
}

impl<C: RaftTypeConfig> Default for StateMachineData<C>
where
    C: RaftTypeConfig<SnapshotData = SnapshotData<C>>,
    C::D: Clone + std::fmt::Debug,
{
    fn default() -> Self {
        Self {
            last_applied: None,
            last_membership: Default::default(),
            data: Default::default(),
        }
    }
}
