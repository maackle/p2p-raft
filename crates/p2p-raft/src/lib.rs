mod dinghy;
mod peer_tracker;
mod types;

pub mod message;
pub mod network;
#[cfg(feature = "testing")]
pub mod testing;

use std::time::Duration;

use openraft::RaftTypeConfig;

pub use dinghy::Dinghy;
pub use peer_tracker::{PeerTracker, PeerTrackerHandle};

#[cfg(feature = "memstore")]
pub mod mem;
#[cfg(feature = "memstore")]
pub use p2p_raft_memstore::{LogStore, StateMachineData, StateMachineStore};

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
pub const ELECTION_TIMEOUT_MIN: Duration = Duration::from_millis(1500);
pub const ELECTION_TIMEOUT_MAX: Duration = Duration::from_millis(3000);

/// If we haven't seen a message from a peer in this long, we consider them unresponsive.
/// The leader will immediately downgrade a node from voter to learner at this time.
pub const RESPONSIVE_INTERVAL: Duration = Duration::from_millis(2000);

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
