#![allow(clippy::uninlined_format_args)]
#![deny(unused_qualifications)]

use std::sync::Arc;
use std::sync::Mutex;

use openraft::Config;
use router::RouterNode;

use crate::app::App;
use crate::router::Router;
use crate::store::Request;
use crate::store::Response;
use crate::store::StateMachineData;

pub mod router;

pub mod api;
pub mod app;
pub mod network;
pub mod store;

pub type NodeId = u64;

openraft::declare_raft_types!(
    /// Declare the type configuration for example K/V store.
    pub TypeConfig:
        D = Request,
        R = Response,
        Node = (),
        // In this example, snapshot is just a copy of the state machine.
        // And it can be any type.
        SnapshotData = StateMachineData,
);

pub type LogStore = store::LogStore;
pub type StateMachineStore = store::StateMachineStore;

pub mod typ;

pub fn encode<T: serde::Serialize>(t: T) -> String {
    serde_json::to_string(&t).unwrap()
}

pub fn decode<T: serde::de::DeserializeOwned>(s: &str) -> T {
    serde_json::from_str(s).unwrap()
}

pub async fn new_raft(node_id: NodeId, router: Arc<Mutex<Router>>) -> (typ::Raft, App) {
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

    let app = App::new(raft.clone(), node, state_machine_store);

    (raft, app)
}
