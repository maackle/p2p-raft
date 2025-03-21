use std::time::Duration;

#[derive(Clone, Debug)]
pub struct Config {
    /// If we haven't seen a message from a peer in this long, we consider them unresponsive.
    /// The leader will immediately downgrade a node from voter to learner at this time.
    pub responsive_interval: Duration,

    /// The interval at which the node will attempt to send join requests to other nodes
    /// when it discovers it is no longer a voter.
    pub join_interval: Duration,

    /// If true, the node will send membership signals.
    /// False by default, because this is currently unstable.
    pub unstable_membership_signals: bool,

    /// Timeout for requests to other nodes.
    pub request_timeout: Duration,

    /// If true, the node will start running automatically.
    /// If false, the node will not start running until `start` is called.
    pub automatic_start: bool,

    /// Config for openraft.
    pub raft_config: openraft::Config,
}

impl Config {
    pub fn testing(heartbeat: u64) -> Self {
        let mut config = Self::default();
        config.join_interval = Duration::from_millis(heartbeat * 6);
        config.responsive_interval = Duration::from_millis(heartbeat * 10);

        config.raft_config.heartbeat_interval = heartbeat;
        config.raft_config.election_timeout_min = heartbeat * 3;
        config.raft_config.election_timeout_max = heartbeat * 6;
        config
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            responsive_interval: Duration::from_millis(3000),
            join_interval: Duration::from_millis(3000),
            request_timeout: Duration::from_millis(5000),
            unstable_membership_signals: false,
            automatic_start: true,
            raft_config: openraft::Config::default(),
        }
    }
}
