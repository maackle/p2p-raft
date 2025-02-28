//! Provide storage layer implementation for examples.

mod log_store;
mod state_machine;

pub use log_store::LogStore;
pub use state_machine::StateMachineStore;

openraft::declare_raft_types!(
    /// Declare the type configuration for example K/V store.
    pub TypeConfig:
        D = Request,
        R = Response,
        Node = (),
        // In this example, snapshot is just a copy of the state machine.
        // And it can be any type.
        SnapshotData = StateMachineData,
);

pub type Request = u64;
pub type Response = ();
pub type NodeId = u64;

/// Data contained in the Raft state machine.
///
/// Note that we are using `serde` to serialize the
/// `data`, which has a implementation to be serialized. Note that for this test we set both the key
/// and value as String, but you could set any type of value that has the serialization impl.
#[derive(serde::Serialize, serde::Deserialize, Default, Debug, Clone)]
pub struct StateMachineData {
    pub last_applied: Option<openraft::LogId<TypeConfig>>,

    pub last_membership: openraft::StoredMembership<TypeConfig>,

    /// Application data, just a list of requests.
    pub data: Vec<Request>,
}
