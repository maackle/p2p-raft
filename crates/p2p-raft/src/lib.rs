mod dinghy;
mod peer_tracker;
mod types;

pub mod message;
pub mod network;
#[cfg(feature = "testing")]
pub mod testing;

use std::time::Duration;

pub use dinghy::Dinghy;
use openraft::RaftTypeConfig;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
pub const ELECTION_TIMEOUT_MIN: Duration = Duration::from_millis(1500);
pub const ELECTION_TIMEOUT_MAX: Duration = Duration::from_millis(3000);

/// If we haven't seen a message from a peer in this long, we consider them unresponsive.
/// The leader will immediately downgrade a node from voter to learner at this time.
pub const RESPONSIVE_INTERVAL: Duration = Duration::from_millis(2_000);

pub trait TypeConf: RaftTypeConfig
where
    Self: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<Self>>,
    Self::SnapshotData: std::fmt::Debug,
    Self::R: std::fmt::Debug,
{
}
