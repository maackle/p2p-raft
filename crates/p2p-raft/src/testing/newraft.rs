use openraft::*;

use super::{NodeId, TypeConfig};

use crate::{
    testing::{Router, RouterNode},
    Dinghy, ELECTION_TIMEOUT_MAX, ELECTION_TIMEOUT_MIN, HEARTBEAT_INTERVAL,
};

impl Router {
    pub async fn new_raft(&self, node_id: NodeId) -> Dinghy<TypeConfig, RouterNode> {
        // Create a configuration for the raft instance.
        let config = Config {
            // snapshot_policy: SnapshotPolicy::LogsSinceLast(0),
            heartbeat_interval: HEARTBEAT_INTERVAL.as_millis() as u64,
            election_timeout_min: ELECTION_TIMEOUT_MIN.as_millis() as u64,
            election_timeout_max: ELECTION_TIMEOUT_MAX.as_millis() as u64,
            // Once snapshot is built, delete the logs at once.
            // So that all further replication will be based on the snapshot.
            max_in_snapshot_log_to_keep: 0,
            ..Default::default()
        };

        let node = RouterNode {
            source: node_id,
            router: self.clone(),
        };

        let dinghy = Dinghy::new_mem(node_id, config, node).await;

        self.lock().targets.insert(node_id, dinghy.clone());

        dinghy
    }
}
