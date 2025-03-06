mod dinghy;
mod peer_tracker;
mod types;

mod config;
pub mod message;
pub mod network;
#[cfg(feature = "testing")]
pub mod testing;

use openraft::RaftTypeConfig;

pub use config::DinghyConfig;
pub use dinghy::Dinghy;
pub use peer_tracker::{PeerTracker, PeerTrackerHandle};

#[cfg(feature = "memstore")]
pub mod mem;
#[cfg(feature = "memstore")]
pub use p2p_raft_memstore::{LogStore, StateMachineData, StateMachineStore};

/// Extra trait bounds on RaftTypeConfig which are generally required by this crate.
pub trait TypeCfg:
    RaftTypeConfig<
    D: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
    R: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
    Entry: Clone + serde::Serialize + serde::de::DeserializeOwned,
    Vote: Clone + serde::Serialize + serde::de::DeserializeOwned,
    LeaderId: Clone + serde::Serialize + serde::de::DeserializeOwned,
    SnapshotData: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
    Node = (),
    Responder = openraft::impls::OneshotResponder<Self>,
>
{
}
