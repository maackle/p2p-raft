use std::{collections::BTreeSet, future::Future, sync::Arc};

use maplit::btreemap;
use openraft::{
    alias::ResponderReceiverOf,
    error::{Fatal, InitializeError, RaftError},
    raft::ClientWriteResult,
    ChangeMembers, Entry, EntryPayload, Raft, Snapshot,
};

use crate::{
    config::DinghyConfig,
    message::{
        P2pError, P2pRequest, P2pResponse, RaftRequest, RaftResponse, RpcRequest, RpcResponse,
    },
    network::P2pNetwork,
    signal::{RaftEvent, SignalSender},
    PeerTrackerHandle, TypeCfg,
};

#[derive(Clone, derive_more::Deref)]
pub struct Dinghy<C: TypeCfg, N: P2pNetwork<C>> {
    #[deref]
    pub raft: Raft<C>,

    pub config: Arc<DinghyConfig>,
    pub id: C::NodeId,
    pub store: p2p_raft_memstore::LogStore<C>,
    pub tracker: PeerTrackerHandle<C>,
    pub network: N,
    pub(crate) signal_tx: Option<SignalSender<C>>,
}

impl<C: TypeCfg, N: P2pNetwork<C>> Dinghy<C, N> {
    pub async fn is_leader(&self) -> bool {
        self.current_leader().await.as_ref() == Some(&self.id)
    }

    pub async fn is_voter(&self, id: &C::NodeId) -> Result<bool, openraft::error::Fatal<C>> {
        let id = id.clone();
        self.raft
            .with_raft_state(move |s| {
                s.membership_state
                    .committed()
                    .voter_ids()
                    .find(|n| *n == id)
                    .is_some()
            })
            .await
    }

    pub async fn initialize(
        &self,
        ids: impl IntoIterator<Item = C::NodeId>,
    ) -> Result<(), RaftError<C, InitializeError<C>>>
    where
        BTreeSet<C::NodeId>: openraft::membership::IntoNodes<C::NodeId, C::Node>,
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
        C: TypeCfg<Entry = Entry<C>>,
    {
        use openraft::storage::RaftLogStorage;
        use openraft::RaftLogReader;

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
    ) -> Result<RpcResponse<C>, Fatal<C>> {
        Ok(match req {
            RpcRequest::P2p(p2p_req) => self.handle_p2p_request(from, p2p_req).await?.into(),
            RpcRequest::Raft(raft_req) => self.handle_raft_request(from, raft_req).await?.into(),
        })
    }

    async fn handle_raft_request(
        &self,
        _from: C::NodeId,
        raft_req: RaftRequest<C>,
    ) -> Result<RaftResponse<C>, Fatal<C>> {
        let res: RaftResponse<C> = match raft_req {
            RaftRequest::Append(req) => {
                // Send signals
                if let Some(tx) = self.signal_tx.as_ref() {
                    for e in req.entries.iter() {
                        let signal = match &e.payload {
                            EntryPayload::Normal(data) => Some(RaftEvent::EntryCommitted {
                                log_id: e.log_id.clone(),
                                data: data.clone(),
                            }),
                            EntryPayload::Membership(m) => Some(RaftEvent::MembershipChanged {
                                log_id: e.log_id.clone(),
                                members: m.voter_ids().collect::<BTreeSet<_>>(),
                            }),
                            _ => None,
                        };
                        if let Some(signal) = signal {
                            tx.send(signal).await.map_err(|_| {
                                tracing::error!("failed to send RaftEvent signal. PANIC!");
                                Fatal::Panicked
                            })?;
                        }
                    }
                }

                match self.append_entries(req).await {
                    Ok(r) => Ok(r.into()),
                    Err(RaftError::APIError(e)) => Ok(e.into()),
                    Err(RaftError::Fatal(e)) => Err(e),
                }
            }
            RaftRequest::Snapshot {
                vote,
                snapshot_meta,
                snapshot_data,
            } => self
                .install_full_snapshot(
                    vote,
                    Snapshot {
                        meta: snapshot_meta,
                        snapshot: snapshot_data,
                    },
                )
                .await
                .map(Into::into),
            RaftRequest::Vote(req) => match self.vote(req).await {
                Ok(r) => Ok(r.into()),
                Err(RaftError::APIError(e)) => Ok(e.into()),
                Err(RaftError::Fatal(e)) => Err(e),
            },
        }?;

        Ok(res)
    }

    pub async fn handle_p2p_request(
        &self,
        from: C::NodeId,
        req: P2pRequest<C>,
    ) -> Result<P2pResponse<C>, Fatal<C>> {
        let from_current_voter = self.is_voter(&from).await?;

        let res = match req {
            P2pRequest::Propose(data) => {
                if !from_current_voter {
                    return Ok(P2pResponse::P2pError(P2pError::NotVoter));
                }
                self.raft.client_write(data).await
            }
            P2pRequest::Join => {
                self.change_membership(
                    ChangeMembers::AddVoters(btreemap![from.clone() => ()]),
                    true,
                )
                .await
            }
            P2pRequest::Leave => {
                self.change_membership(ChangeMembers::RemoveVoters([from.clone()].into()), true)
                    .await
            }
        };

        Ok(match res {
            Ok(_) => P2pResponse::Ok,
            Err(e) => P2pResponse::RaftError(e),
        })
    }

    pub async fn chore_loop(self) {
        let source = self.id.clone();
        let dinghy = self.clone();

        let mut interval = tokio::time::interval(self.config.p2p_config.join_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if let Some(leader) = dinghy.current_leader().await {
                let is_leader = leader == source;

                let is_voter = if let Ok(l) = dinghy.is_voter(&source).await {
                    l
                } else {
                    continue;
                };

                if is_leader || is_voter {
                    continue;
                }

                // if there is a leader and I'm not a voter, ask to rejoin the cluster
                match dinghy
                    .network
                    .send_p2p(source.clone(), leader, P2pRequest::Join)
                    .await
                {
                    Ok(P2pResponse::Ok) => {}
                    r => {
                        tracing::error!("failed to send join request to leader: {r:?}");
                    }
                }
            }
        }
    }
}

#[cfg(feature = "testing")]
impl<C: TypeCfg, N: P2pNetwork<C>> Dinghy<C, N>
where
    C: TypeCfg<Entry = Entry<C>, SnapshotData = p2p_raft_memstore::StateMachineData<C>>,
{
    #[allow(unused_variables)]
    pub async fn debug_line(&self) -> String {
        use itertools::Itertools;
        let t = self.tracker.lock().await;
        let peers = t.responsive_peers(self.config.p2p_config.responsive_interval);
        let members = self
            .raft
            .with_raft_state(|s| {
                s.membership_state
                    .committed()
                    .voter_ids()
                    .collect::<BTreeSet<_>>()
            })
            .await
            .unwrap();

        let log = self.read_log_data().await;
        let snapshot = self
            .raft
            .get_snapshot()
            .await
            .unwrap()
            .map(|s| s.snapshot.data);

        let lines = [
            format!("... "),
            format!("{}", self.id),
            format!("<{:?}>", self.current_leader().await),
            format!("members {:?}", members),
            format!("sees {:?}", peers),
            // format!("snapshot {:?}", snapshot),
            // format!("log {:?}", log),
        ];

        lines.into_iter().join(" ")
    }
}
