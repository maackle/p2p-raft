mod peer_tracker;
mod raft;
mod types;

mod config;
pub mod message;
pub mod network;
pub mod signal;
#[cfg(feature = "testing")]
pub mod testing;
mod error;

use openraft::RaftTypeConfig;

pub use config::Config;
use p2p_raft_memstore::MemTypeConfig;
pub use peer_tracker::{PeerTracker, PeerTrackerHandle};
pub use raft::{LogOp, P2pRaft};
pub use error::{P2pRaftError as Error, *};

#[cfg(feature = "memstore")]
pub mod mem;
#[cfg(feature = "memstore")]
pub use p2p_raft_memstore::{ArcStateMachineStore, LogStore, StateMachineData, StateMachineStore};

/// Extra trait bounds on RaftTypeConfig which are generally required by this crate.
pub trait TypeCfg:
    serde::Serialize + serde::de::DeserializeOwned + std::fmt::Debug + Clone + Eq + Ord + Send +
    MemTypeConfig +
    RaftTypeConfig<
    D: std::fmt::Debug + Clone + Eq + Ord + serde::Serialize + serde::de::DeserializeOwned,
    // R: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
    Vote: Clone + serde::Serialize + serde::de::DeserializeOwned,
    LeaderId: Clone + serde::Serialize + serde::de::DeserializeOwned,
    Node: Eq + Ord,
    // SnapshotData: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
    
    R = (),
    Entry = openraft::Entry<Self>,
    // Node = (),
    Responder = openraft::impls::OneshotResponder<Self>,
>
{
}
