mod dinghy;
mod peer_tracker;
mod types;

pub mod message;
pub mod network;
#[cfg(feature = "testing")]
pub mod testing;

use std::time::Duration;

pub use dinghy::Dinghy;
use memstore::TypeConfig;
use openraft::RaftTypeConfig;

pub const HEARTBEAT_INTERVAL: Duration = Duration::from_millis(500);
pub const ELECTION_TIMEOUT_MIN: Duration = Duration::from_millis(1500);
pub const ELECTION_TIMEOUT_MAX: Duration = Duration::from_millis(3000);

/// If we haven't seen a message from a peer in this long, we consider them unresponsive.
/// The leader will immediately downgrade a node from voter to learner at this time.
pub const RESPONSIVE_INTERVAL: Duration = Duration::from_millis(2000);

// XXX: wowzers this is ugly. We want a blanked impl to impose more trait bounds on RaftTypeConfig,
// we we don't need them on every function. I couldn't get it to work so I did this for now.
pub trait TypeConf<
    R = (),
    SnapshotData = <TypeConfig as RaftTypeConfig>::SnapshotData,
    Responder = openraft::impls::OneshotResponder<Self>,
>: RaftTypeConfig<R = R, SnapshotData = SnapshotData, Responder = Responder>
{
}
// pub trait TypeConf: RaftTypeConfig
// where
//     // <Self as RaftTypeConfig>::R: std::fmt::Debug,
//     // Self::R: std::fmt::Debug,
//     Self: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<Self>>,
//     Self::SnapshotData: std::fmt::Debug,
//     <Self as RaftTypeConfig>::SnapshotData: std::fmt::Debug,
// {
// }

impl
    TypeConf<
        <TypeConfig as RaftTypeConfig>::R,
        <TypeConfig as RaftTypeConfig>::SnapshotData,
        openraft::impls::OneshotResponder<TypeConfig>,
    > for TypeConfig
{
}

// impl<T> TypeConf for T
// where
//     T: RaftTypeConfig<Responder = openraft::impls::OneshotResponder<T>>,
//     T::SnapshotData: std::fmt::Debug,
//     T::R: std::fmt::Debug,
// {
// }
