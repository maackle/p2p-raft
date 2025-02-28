mod dinghy;
mod peer_tracker;
mod types;

#[cfg(feature = "testing")]
pub mod testing;

use std::time::Duration;

pub use dinghy::Dinghy;

pub const RESPONSIVE_INTERVAL: Duration = Duration::from_millis(5000);
