mod network;
mod newraft;
mod router;
mod utils;

pub use network::*;
pub use router::*;
pub use utils::*;

pub type NodeId = u64;

openraft::declare_raft_types!(
    /// Declare the type configuration for example K/V store.
    #[derive(serde::Serialize, serde::Deserialize)]
    pub TypeConfig:
        D = u64,
        R = (),
        Node = (),
        // In this example, snapshot is just a copy of the state machine.
        // And it can be any type.
        SnapshotData = p2p_raft_memstore::StateMachineData<TypeConfig>,
);

impl crate::TypeCfg for TypeConfig {}
