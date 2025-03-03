use std::{borrow::Borrow, collections::BTreeSet, fmt::Debug, sync::Arc, time::Duration};

use futures::FutureExt;
use itertools::Itertools;
use memstore::StateMachineStore;
use openraft::{
    ChangeMembers, Entry, EntryPayload, Raft, RaftTypeConfig, SnapshotMeta,
    alias::ResponderReceiverOf,
    error::{ClientWriteError, InitializeError, RaftError},
    impls::OneshotResponder,
    raft::ClientWriteResult,
};
use tokio::{sync::Mutex, task::JoinHandle};

use crate::{
    TypeConf,
    message::{P2pRequest, RaftRequest, RaftResponse, RpcRequest, RpcResponse},
    network::P2pNetwork,
    peer_tracker::PeerTracker,
    testing::Router,
};

const CHORE_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, derive_more::From, derive_more::Deref)]
pub struct Dinghy<C: TypeConf, N: P2pNetwork<C> = Router<C>> {
    #[deref]
    pub raft: Raft<C>,

    pub id: C::NodeId,
    pub store: memstore::LogStore<C>,
    pub tracker: Arc<Mutex<PeerTracker<C>>>,
    pub network: N,
}

impl<C: TypeConf> Dinghy<C> {
    pub fn new(
        id: C::NodeId,
        raft: Raft<C>,
        store: memstore::LogStore<C>,
        // TODO: generic
        network: Router<C>,
    ) -> Self {
        Self {
            id,
            raft,
            store,
            tracker: PeerTracker::new(),
            network,
        }
    }

    pub async fn is_leader(&self) -> bool {
        self.current_leader().await.as_ref() == Some(&self.id)
    }

    pub async fn initialize(
        &self,
        ids: impl IntoIterator<Item = C::NodeId>,
    ) -> Result<(), RaftError<C, InitializeError<C>>>
    where
        BTreeSet<<C as RaftTypeConfig>::NodeId>: openraft::membership::IntoNodes<
                <C as RaftTypeConfig>::NodeId,
                <C as RaftTypeConfig>::Node,
            >,
    {
        let ids: BTreeSet<C::NodeId> = ids.into_iter().collect::<BTreeSet<_>>();
        match self.raft.initialize(ids).await {
            Ok(_) => Ok(()),
            // this error is ok, it means we got some network messages already
            Err(RaftError::APIError(InitializeError::NotAllowed(_))) => Ok(()),
            e => e,
        }
    }

    pub async fn read_log_data(&self) -> anyhow::Result<Vec<C::D>>
    where
        C: TypeConf<Entry = Entry<C>>,
        C::Entry: Clone,
    {
        use openraft::RaftLogReader;
        use openraft::storage::RaftLogStorage;

        Ok(self
            .store
            .clone()
            .get_log_reader()
            .await
            .try_get_log_entries(..)
            .await?
            .into_iter()
            .filter_map(|e: Entry<C>| match e.payload {
                EntryPayload::Normal(n) => Some(n),
                _ => None,
            })
            .collect::<Vec<_>>())
    }

    pub async fn write_linearizable<E>(&self, data: C::D) -> anyhow::Result<()>
    where
        ResponderReceiverOf<C>: Future<Output = Result<ClientWriteResult<C>, E>>,
        E: std::error::Error + openraft::OptionalSend,
    {
        self.ensure_linearizable().await?;
        self.raft.client_write(data).await?;
        Ok(())
    }

    pub async fn handle_request(
        &self,
        from: C::NodeId,
        req: RpcRequest<C>,
    ) -> Result<RpcResponse<C>, openraft::error::Unreachable> {
        // TODO: handle errors?
        let res = match req {
            RpcRequest::P2p(p2p_req) => {
                self.handle_p2p_request(from, p2p_req).await?;
                RpcResponse::Ok
            }
            RpcRequest::Raft(raft_req) => {
                RpcResponse::Raft(self.handle_raft_request(from, raft_req).await?)
            }
        };
        Ok(res)
    }

    async fn handle_raft_request(
        &self,
        _from: C::NodeId,
        raft_req: RaftRequest<C>,
    ) -> Result<RaftResponse<C>, openraft::error::Unreachable> {
        Ok(match raft_req {
            RaftRequest::Append(req) => self
                .append_entries(req)
                .await
                .map_err(|e| openraft::error::Unreachable::new(&e))?
                .into(),
            RaftRequest::Snapshot { vote, snapshot } => self
                .install_full_snapshot(vote, snapshot)
                .await
                .map_err(|e| openraft::error::Unreachable::new(&e))?
                .into(),
            RaftRequest::Vote(req) => self
                .vote(req)
                .await
                .map_err(|e| openraft::error::Unreachable::new(&e))?
                .into(),
        })
    }

    pub async fn handle_p2p_request(
        &self,
        from: C::NodeId,
        req: P2pRequest,
    ) -> Result<(), openraft::error::Unreachable> {
        let i_am_leader = self.current_leader().await.as_ref() == Some(&self.id);
        let from2 = from.clone();
        let from_current_voter = self
            .raft
            .with_raft_state(move |s| s.membership_state.committed().voter_ids().contains(&from2))
            .await
            .unwrap();

        if i_am_leader && !from_current_voter {
            let res = match req {
                P2pRequest::Join => {
                    self.change_membership(ChangeMembers::AddVoterIds([from.clone()].into()), true)
                        .await
                }
                P2pRequest::Leave => {
                    self.change_membership(ChangeMembers::RemoveVoters([from.clone()].into()), true)
                        .await
                }
            };

            match res {
                Err(RaftError::APIError(ClientWriteError::ForwardToLeader(_))) => {
                    // OK, we're not a leader, but the leader will take care of it.
                }
                Err(RaftError::APIError(ClientWriteError::ChangeMembershipError(
                    openraft::error::ChangeMembershipError::InProgress(_),
                ))) => {
                    // OK, we're working on it!
                }
                Err(e) => {
                    // This is a legit problem!
                    println!("*** add voter error: {e:?}");
                    return Err(openraft::error::Unreachable::new(&e));
                }
                Ok(_) => {
                    println!("*** added voter {from} to this raft");
                    // good!
                }
            };
        }
        Ok(())
    }

    pub fn spawn_chore_loop(&self) -> JoinHandle<()> {
        let source = self.id.clone();
        let raft = self.raft.clone();
        let network = self.network.clone();

        let mut interval = tokio::time::interval(CHORE_INTERVAL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        tokio::spawn(async move {
            loop {
                interval.tick().await;

                if let Some(leader) = raft.current_leader().await {
                    let is_leader = leader == source;

                    let source2 = source.clone();
                    let is_voter = raft
                        .with_raft_state(move |s| {
                            s.membership_state
                                .committed()
                                .voter_ids()
                                .contains(&source2)
                        })
                        .await
                        .unwrap();

                    println!("{source}: {is_leader} {is_voter}");

                    if is_leader || is_voter {
                        continue;
                    }

                    // if there is a leader and I'm not a voter, ask to rejoin the cluster
                    match network.send(source.clone(), leader, P2pRequest::Join).await {
                        Ok(_) => {}
                        Err(e) => {
                            println!("failed to send join request to leader: {e:?}");
                        }
                    }
                }
            }
        })
    }
}

impl Dinghy<memstore::TypeConfig> {
    /// NOTE: only run this as leader!
    /// XXX: really this is just a workaround for when it's not feasible to implement
    ///      merging in the state machine, when that logic needs to be in the front end
    ///      and the merged snapshot is forced by the leader.
    #[deprecated = "this does not work!"]
    pub async fn replace_snapshot(&self, data: Vec<memstore::Request>) {
        use openraft::storage::{RaftSnapshotBuilder, RaftStateMachine};

        let smd = {
            let mut sm = self
                .raft
                .with_state_machine(|s: &mut Arc<StateMachineStore<memstore::TypeConfig>>| {
                    async move { s.state_machine.lock().unwrap().clone() }.boxed()
                })
                .await
                .unwrap()
                .unwrap();

            sm.data = data;
            // sm.last_applied.map(|mut l| {
            //     l.index += 1;
            //     l
            // });
            sm
        };

        let snapshot = Box::new(smd.clone());

        let snapshot_id = self
            .raft
            .with_raft_state(|s| s.snapshot_meta.snapshot_id.clone())
            .await
            .unwrap();

        let meta = SnapshotMeta {
            last_log_id: smd.last_applied,
            last_membership: smd.last_membership,
            snapshot_id,
            // snapshot_id: nanoid::nanoid!(),
        };

        self.with_state_machine(
            move |s: &mut Arc<StateMachineStore<memstore::TypeConfig>>| {
                async move {
                    s.clone()
                        .install_snapshot(&meta.clone(), snapshot)
                        .await
                        .unwrap();
                    // s.build_snapshot().await.unwrap();
                }
                .boxed()
            },
        )
        .await
        .unwrap()
        .unwrap();

        let trigger = self.raft.trigger();
        trigger
            .purge_log(smd.last_applied.map(|l| l.index).unwrap_or_default())
            .await
            .unwrap();
        trigger.snapshot().await.unwrap();
        trigger.heartbeat().await.unwrap();

        // raft.install_full_snapshot(Vote::new(term, raft.id), snapshot)
        //     .await
        //     .unwrap();
    }
}
