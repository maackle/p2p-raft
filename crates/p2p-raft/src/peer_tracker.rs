use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use openraft::{
    ChangeMembers,
    error::{ClientWriteError, RaftError},
};
use tokio::{sync::Mutex, time::Instant};

use crate::{Dinghy, TypeConf, network::P2pNetwork};

pub struct PeerTracker<C: TypeConf> {
    last_seen: BTreeMap<C::NodeId, Instant>,
}

impl<C: TypeConf> PeerTracker<C> {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            last_seen: Default::default(),
        }))
    }

    pub fn touch(&mut self, node: &C::NodeId) {
        self.last_seen.insert(node.clone(), Instant::now());
    }

    pub async fn handle_absentees(&mut self, raft: &Dinghy<C>, interval: Duration) {
        // TODO: don't skip!
        // return;
        let unresponsive = self.unresponsive_members(raft, interval).await;
        if !unresponsive.is_empty() && raft.is_leader().await {
            match raft
                .change_membership(ChangeMembers::RemoveVoters(unresponsive.clone()), true)
                .await
            {
                Ok(_)
                | Err(RaftError::APIError(ClientWriteError::ChangeMembershipError(
                    openraft::error::ChangeMembershipError::InProgress(_),
                ))) => {
                    println!("*** removing absentees: {:?}", unresponsive);
                    // XXX: this hack ensures that we only attempt removing nodes once per PRESENCE_WINDOW.
                    //      if they are removed by the next time the interval expires, they won't show up
                    //      in the next unresponsive set.
                    for p in unresponsive.iter() {
                        self.touch(p);
                    }
                }
                Err(RaftError::APIError(ClientWriteError::ForwardToLeader(_))) => {
                    // minor race condition, it's ok
                }
                e => {
                    println!("*** ERROR: Failed to remove absentees: {}, {e:?}", raft.id);
                }
            }
        }
    }

    /// Returns the set of peers that have been seen in the last `interval` seconds.
    /// NOTE, this does not include the local node.
    pub fn responsive_peers(&self, interval: Duration) -> BTreeSet<C::NodeId> {
        self.last_seen
            .iter()
            .filter(|(_, t)| t.elapsed() < interval)
            .map(|(to, _)| C::NodeId::from(to.clone()))
            .collect()
    }

    async fn unresponsive_members(
        &self,
        raft: &Dinghy<C>,
        interval: Duration,
    ) -> BTreeSet<C::NodeId> {
        let here = self.responsive_peers(interval);

        let mut all_members = raft
            .with_raft_state(move |s| {
                s.membership_state
                    .committed()
                    // .effective()
                    .voter_ids()
                    .collect::<BTreeSet<_>>()
            })
            .await
            .unwrap_or_default();

        all_members.remove(&raft.id);

        all_members.difference(&here).cloned().collect()
    }

    pub fn last_seen(&self) -> &BTreeMap<C::NodeId, Instant> {
        &self.last_seen
    }
}
