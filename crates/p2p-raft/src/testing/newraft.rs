use std::sync::Arc;

use super::{NodeId, TypeConfig};

use crate::{
    config::Config,
    signal::SignalSender,
    testing::{Router, RouterNode},
    P2pRaft,
};

pub fn standard_raft_config() -> openraft::Config {
    openraft::Config {
        heartbeat_interval: 500,
        election_timeout_min: 1500,
        election_timeout_max: 3000,
        // max_in_snapshot_log_to_keep: 0,
        ..Default::default()
    }
}

impl Router {
    pub async fn new_raft(
        &self,
        node_id: NodeId,
        config: impl Into<Arc<Config>>,
        signal_tx: Option<SignalSender<TypeConfig>>,
    ) -> P2pRaft<TypeConfig, RouterNode> {
        let node = RouterNode {
            source: node_id,
            router: self.clone(),
        };

        let raft = P2pRaft::new_mem(node_id, config, node, signal_tx, |_| ()).await;

        self.lock().targets.insert(node_id, raft.clone());

        raft
    }
}
