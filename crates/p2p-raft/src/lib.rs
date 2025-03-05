mod dinghy;
mod peer_tracker;
mod types;

pub mod message;
pub mod network;
#[cfg(feature = "testing")]
pub mod testing;

use std::time::Duration;

pub use dinghy::Dinghy;
pub use memstore::{LogStore, StateMachineStore};
use openraft::RaftTypeConfig;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
pub const ELECTION_TIMEOUT_MIN: Duration = Duration::from_millis(1500);
pub const ELECTION_TIMEOUT_MAX: Duration = Duration::from_millis(3000);

/// If we haven't seen a message from a peer in this long, we consider them unresponsive.
/// The leader will immediately downgrade a node from voter to learner at this time.
pub const RESPONSIVE_INTERVAL: Duration = Duration::from_millis(2000);

pub trait TypeConf:
    RaftTypeConfig<
    D: std::fmt::Debug + Clone,
    R: std::fmt::Debug + Clone,
    Entry: Clone,
    Vote: Clone,
    LeaderId: Clone,
    SnapshotData: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
    Responder = openraft::impls::OneshotResponder<Self>,
>
{
}

// impl<T> TypeConf for T
// where
//     T: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<T>>,
//     T::D: std::fmt::Debug + Clone,
//     T::Entry: Clone,
//     T::Vote: Clone,
//     T::LeaderId: Clone,
//     T::SnapshotData: std::fmt::Debug + Clone + serde::Serialize + serde::de::DeserializeOwned,
// {
// }

impl TypeConf for memstore::TypeConfig {}
