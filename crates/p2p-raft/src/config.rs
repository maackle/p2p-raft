use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct DinghyConfig {
    pub raft_config: openraft::Config,
    pub p2p_config: P2pConfig,
}

impl DinghyConfig {
    pub fn testing(heartbeat: u64) -> Self {
        let mut config = Self::default();
        config.p2p_config.join_interval = Duration::from_millis(heartbeat * 6);
        config.p2p_config.responsive_interval = Duration::from_millis(heartbeat * 6);

        config.raft_config.heartbeat_interval = heartbeat;
        config.raft_config.election_timeout_min = heartbeat * 3;
        config.raft_config.election_timeout_max = heartbeat * 6;
        config
    }
}

#[derive(Clone, Debug)]
pub struct P2pConfig {
    /// If we haven't seen a message from a peer in this long, we consider them unresponsive.
    /// The leader will immediately downgrade a node from voter to learner at this time.
    pub responsive_interval: Duration,

    /// The interval at which the node will attempt to send join requests to other nodes
    /// when it discovers it is no longer a voter.
    pub join_interval: Duration,

    /// If true, the node will send membership signals.
    /// False by default, because this is currently unstable.
    pub unstable_membership_signals: bool,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            responsive_interval: Duration::from_millis(3000),
            join_interval: Duration::from_millis(3000),
            unstable_membership_signals: false,
        }
    }
}
