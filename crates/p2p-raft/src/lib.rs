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

// XXX: would be nice if these type bounds actually worked, but I can't get them to.
//      so they are littered around all the impls.
pub trait TypeConf: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<Self>>
// where
//     // Self: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<Self>>,
//     // Self::SnapshotData: std::fmt::Debug,
//     // Self::R: std::fmt::Debug,
//     <Self as RaftTypeConfig>::SnapshotData: std::fmt::Debug,
//     <Self as RaftTypeConfig>::R: std::fmt::Debug,
{
}

impl<T> TypeConf for T where T: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<T>> {}
