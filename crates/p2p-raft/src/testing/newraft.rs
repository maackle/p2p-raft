use std::sync::Arc;

use openraft::*;

use super::{NodeId, TypeConfig};

use crate::{
    config::DinghyConfig,
    signal::SignalSender,
    testing::{Router, RouterNode},
    Dinghy,
};

pub fn standard_raft_config() -> Config {
    Config {
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
        config: impl Into<Arc<DinghyConfig>>,
        signal_tx: Option<SignalSender<TypeConfig>>,
    ) -> Dinghy<TypeConfig, RouterNode> {
        let node = RouterNode {
            source: node_id,
            router: self.clone(),
        };

        let dinghy = Dinghy::new_mem(node_id, config, node, signal_tx).await;

        self.lock().targets.insert(node_id, dinghy.clone());

        dinghy
    }
}
