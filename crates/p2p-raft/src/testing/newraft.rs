use std::sync::Arc;

use openraft::*;

use memstore::{LogStore, NodeId, TypeConfig};

use crate::{
    Dinghy,
    testing::{Router, RouterNode},
};

impl Router<TypeConfig> {
    pub async fn new_raft(&self, node_id: NodeId) -> Dinghy<TypeConfig> {
        // Create a configuration for the raft instance.
        let config = Config {
            // snapshot_policy: SnapshotPolicy::LogsSinceLast(0),
            heartbeat_interval: 500,
            election_timeout_min: 1500,
            election_timeout_max: 3000,
            // Once snapshot is built, delete the logs at once.
            // So that all further replication will be based on the snapshot.
            max_in_snapshot_log_to_keep: 0,
            ..Default::default()
        };

        let config = Arc::new(config.validate().unwrap());

        // Create a instance of where the Raft logs will be stored.
        let log_store = LogStore::default();

        // Create a instance of where the state machine data will be stored.
        let state_machine_store = Arc::new(memstore::StateMachineStore::default());

        let node = RouterNode {
            source: node_id,
            router: self.clone(),
        };

        // Create a local raft instance.
        let raft = openraft::Raft::new(
            node_id,
            config,
            node.clone(),
            log_store.clone(),
            state_machine_store.clone(),
        )
        .await
        .unwrap();

        let dinghy = Dinghy::new(node_id, raft, log_store);

        self.lock().await.targets.insert(node_id, dinghy.clone());

        dinghy
    }
}
