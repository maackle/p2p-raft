use std::sync::Arc;

use openraft::*;

use network_impl::{LogStore, StateMachineStore, TypeConfig, app::App, typ};

use crate::router::{Router, RouterNode};

pub async fn new_raft(node_id: u64, router: Router<TypeConfig>) -> (typ::Raft, App) {
    // Create a configuration for the raft instance.
    let config = Config {
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
    let state_machine_store = Arc::new(StateMachineStore::default());

    let node = RouterNode {
        source: node_id,
        router,
    };

    // Create a local raft instance.
    let raft = openraft::Raft::new(
        node_id,
        config,
        node.clone(),
        log_store,
        state_machine_store.clone(),
    )
    .await
    .unwrap();

    // let app = App::new(raft.clone(), node, state_machine_store);
    let app = todo!();

    (raft, app)
}
