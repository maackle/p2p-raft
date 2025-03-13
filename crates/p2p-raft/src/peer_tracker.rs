use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use openraft::{
    error::{ClientWriteError, RaftError},
    ChangeMembers,
};
use tokio::{sync::Mutex, time::Instant};

use crate::{network::P2pNetwork, P2pRaft, TypeCfg};

#[derive(Clone, derive_more::Deref)]
pub struct PeerTrackerHandle<C: TypeCfg>(Arc<Mutex<PeerTracker<C>>>);

impl<C: TypeCfg> PeerTrackerHandle<C> {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(PeerTracker::default())))
    }
}

#[derive(Default)]
pub struct PeerTracker<C: TypeCfg> {
    last_seen: BTreeMap<C::NodeId, Instant>,
}

impl<C: TypeCfg> PeerTracker<C> {
    pub fn touch(&mut self, node: &C::NodeId) {
        self.last_seen.insert(node.clone(), Instant::now());
    }

    pub async fn handle_absentees<N: P2pNetwork<C>>(
        &mut self,
        raft: &P2pRaft<C, N>,
        interval: Duration,
    ) {
        if !raft.is_leader().await {
            // XXX: this is an easy way to ensure that when we switch to being a leader,
            //      we don't immediately remove all nodes, because we weren't talking to them
            //      while there was a different leader.
            self.last_seen.values_mut().for_each(|t| {
                *t = Instant::now();
            });

            return;
        }

        let unresponsive = self.unresponsive_members(raft, interval).await;
        if !unresponsive.is_empty() {
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

    async fn unresponsive_members<N: P2pNetwork<C>>(
        &self,
        raft: &P2pRaft<C, N>,
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
