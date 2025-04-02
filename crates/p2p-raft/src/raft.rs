use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::Arc,
};

use anyhow::anyhow;
use futures::{stream::FuturesUnordered, StreamExt};
use itertools::Itertools;
use maplit::btreemap;
use openraft::{
    alias::{LogIdOf, ResponderReceiverOf},
    error::{ClientWriteError, InitializeError, RaftError},
    metrics::WaitError,
    raft::ClientWriteResult,
    ChangeMembers, Entry, EntryPayload, Snapshot,
};
use p2p_raft_memstore::ArcStateMachineStore;
use tokio_util::sync::CancellationToken;

use crate::{
    config::Config,
    error::{P2pRaftError, PResult},
    message::{P2pError, P2pRequest, P2pResponse, RaftRequest, RaftResponse, Request, Response},
    network::P2pNetwork,
    signal::{RaftEvent, SignalSender},
    PeerTrackerHandle, TypeCfg,
};

#[derive(Clone, derive_more::Deref)]
pub struct P2pRaft<C: TypeCfg, N: P2pNetwork<C>> {
    #[deref]
    pub raft: openraft::Raft<C>,

    pub id: C::NodeId,
    pub config: Arc<Config>,
    pub store: p2p_raft_memstore::LogStore<C>,
    pub network: N,
    pub tracker: PeerTrackerHandle<C>,
    pub(crate) nodemap: Arc<dyn Fn(C::NodeId) -> C::Node + Send + Sync + 'static>,

    signal_tx: Option<SignalSender<C>>,
    cancel: CancellationToken,
}

impl<C: TypeCfg, N: P2pNetwork<C>> P2pRaft<C, N> {
    pub async fn spawn(
        node_id: C::NodeId,
        config: impl Into<Arc<Config>>,
        log_store: p2p_raft_memstore::LogStore<C>,
        state_machine_store: p2p_raft_memstore::StateMachineStore<C>,
        network: N,
        signal_tx: Option<SignalSender<C>>,
        nodemap: impl Fn(C::NodeId) -> C::Node + Send + Sync + 'static,
    ) -> anyhow::Result<Self>
    where
        C: TypeCfg<R = (), Entry = Entry<C>>,
    {
        let cancel = CancellationToken::new();
        let config = config.into();

        let raft = openraft::Raft::new(
            node_id.clone(),
            config.raft_config.clone().into(),
            network.clone(),
            log_store.clone(),
            ArcStateMachineStore::from(Arc::new(state_machine_store)),
        )
        .await?;

        let raft = Self {
            raft,
            id: node_id,
            config,
            store: log_store,
            network,
            tracker: PeerTrackerHandle::new(),
            nodemap: Arc::new(nodemap),
            signal_tx,
            cancel,
        };

        raft.start();

        Ok(raft)
    }

    pub fn start(&self) {
        {
            let token = self.cancel.clone();
            let chores = self.clone().chore_loop();
            tokio::spawn(async move { token.run_until_cancelled(chores).await });
        }
        if let Some(tx) = self.signal_tx.clone() {
            let token = self.cancel.clone();
            let chores = self.clone().signal_loop(tx);
            tokio::spawn(async move { token.run_until_cancelled(chores).await });
        }
    }

    pub async fn is_leader(&self) -> bool {
        self.current_leader().await.as_ref() == Some(&self.id)
    }

    pub async fn is_voter(&self, id: &C::NodeId) -> anyhow::Result<bool> {
        let id = id.clone();
        Ok(self
            .raft
            .with_raft_state(move |s| {
                s.membership_state
                    .committed()
                    .voter_ids()
                    .find(|n| *n == id)
                    .is_some()
            })
            .await?)
    }

    pub async fn initialize(&self, ids: impl IntoIterator<Item = C::NodeId>) -> anyhow::Result<()>
    where
        BTreeSet<C::NodeId>: openraft::membership::IntoNodes<C::NodeId, C::Node>,
    {
        let ids: BTreeSet<C::NodeId> = ids.into_iter().collect::<BTreeSet<_>>();
        match self.raft.initialize(ids).await {
            Ok(_) => Ok(()),
            // this error is ok, it means we got some network messages already
            Err(RaftError::APIError(InitializeError::NotAllowed(_))) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn broadcast_join(&self, ids: impl IntoIterator<Item = C::NodeId>) -> PResult<(), C> {
        let ids: BTreeSet<C::NodeId> = ids.into_iter().collect();

        // If only self was given, we can't join the cluster
        if ids.is_empty() {
            return Err(
                anyhow::anyhow!("Can't use `broadcast_join` with an empty set of nodes.").into(),
            );
        }

        let network = self.network.clone();
        let mut futs: FuturesUnordered<_> = ids
            .into_iter()
            .filter(|id| *id != self.id)
            .map(|id| network.send_rpc(id, P2pRequest::Join))
            .collect();

        // If only self was given, we can't join the cluster
        if futs.is_empty() {
            return Err(anyhow::anyhow!("Can't use `broadcast_join` with only self.").into());
        }

        let mut forwards = BTreeMap::new();
        let mut errors = Vec::new();

        while let Some(res) = futs.next().await {
            match res {
                Ok(res) => {
                    if res.is_ok() {
                        // Return early if we successfully reached the leader and joined
                        return Ok(());
                    } else {
                        if let Some(forward) = res.forward_to_leader() {
                            *forwards.entry(forward).or_insert(0) += 1;
                        } else {
                            errors.push(anyhow!("p2p error joining: {res:?}"));
                        }
                    }
                }
                Err(e) => errors.push(e),
            }
        }

        if let Some((forward, _)) = forwards
            .into_iter()
            .max_by_key(|(fwd, count)| (*count, fwd.is_some()))
        {
            Err(P2pRaftError::NotLeader(forward))
        } else {
            Err(anyhow::anyhow!("Failed to join any nodes. All errors: {:?}", errors).into())
        }
    }

    pub async fn leave(&self) -> anyhow::Result<()> {
        self.send_rpc_to_leader_with_retry(P2pRequest::Leave)
            .await?
            .to_anyhow()
    }

    pub async fn propose(&self, data: C::D) -> anyhow::Result<()> {
        self.send_rpc_to_leader_with_retry(P2pRequest::Propose(data))
            .await?
            .to_anyhow()
    }

    pub async fn read_log_data(&self, start_index: u64) -> anyhow::Result<Vec<C::D>>
    where
        C: TypeCfg<Entry = Entry<C>>,
    {
        Ok(self
            .read_log_ops(start_index)
            .await?
            .into_iter()
            .map(|i| i.op)
            .collect_vec())
    }

    pub async fn read_log_ops(&self, start_index: u64) -> anyhow::Result<Vec<LogOp<C>>>
    where
        C: TypeCfg<Entry = Entry<C>>,
    {
        Ok(self
            .read_log_entries(start_index)
            .await?
            .into_iter()
            .filter_map(|e: Entry<C>| match e.payload {
                EntryPayload::Normal(n) => Some(LogOp {
                    log_id: e.log_id.clone(),
                    op: n.clone(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>())
    }

    pub async fn read_log_entries(&self, start_index: u64) -> anyhow::Result<Vec<C::Entry>>
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
            .try_get_log_entries(start_index..)
            .await?)
    }

    pub async fn write_data(
        &self,
        data: C::D,
    ) -> Result<LogIdOf<C>, RaftError<C, ClientWriteError<C>>> {
        let r = self.raft.client_write(data.clone()).await?;
        Ok(r.log_id)
    }

    pub async fn write_linearizable<E>(&self, data: C::D) -> anyhow::Result<()>
    where
        ResponderReceiverOf<C>: Future<Output = Result<ClientWriteResult<C>, E>>,
        E: std::error::Error + openraft::OptionalSend,
    {
        self.ensure_linearizable().await?;
        self.write_data(data).await?;
        Ok(())
    }

    pub async fn send_rpc_to_leader_with_retry(
        &self,
        req: P2pRequest<C>,
    ) -> anyhow::Result<P2pResponse<C>> {
        let retries = 3;
        let mut leader = self.try_current_leader().await?;
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        for i in 0..retries {
            tracing::info!("retry {i}, leader={leader:?}");

            interval.tick().await;

            if leader == self.id {
                return Ok(self
                    .handle_rpc(self.id.clone(), req.clone().into())
                    .await?
                    .unwrap_p_2_p());
            } else {
                let res = match tokio::time::timeout(
                    self.config.request_timeout,
                    self.network.send_rpc(leader, req.clone()),
                )
                .await
                {
                    Ok(r) => r?,
                    Err(_) => {
                        tracing::error!("request timed out");
                        anyhow::bail!("request timed out")
                    }
                };

                if let Some(forward) = res.forward_to_leader() {
                    if let Some((new_leader, _node)) = forward {
                        leader = new_leader;
                    } else {
                        leader = self.try_current_leader().await?;
                    }
                } else {
                    return Ok(res);
                }
            }
        }

        anyhow::bail!("could not find leader");
    }

    async fn try_current_leader(&self) -> anyhow::Result<C::NodeId> {
        match self.current_leader().await {
            Some(leader) => Ok(leader),
            None => anyhow::bail!("no leader available"),
        }
    }

    pub async fn handle_rpc(
        &self,
        from: C::NodeId,
        req: Request<C>,
    ) -> anyhow::Result<Response<C>> {
        // TODO: avoid clone
        let res: Response<C> = match req.clone() {
            Request::P2p(p2p_req) => self.handle_p2p_request(from, p2p_req).await?.into(),
            Request::Raft(raft_req) => self.handle_raft_request(from, raft_req).await?.into(),
        };

        Ok(res)
    }

    pub(crate) async fn handle_raft_request(
        &self,
        _from: C::NodeId,
        raft_req: RaftRequest<C>,
    ) -> anyhow::Result<RaftResponse<C>> {
        let res: RaftResponse<C> = match raft_req {
            RaftRequest::Append(req) => match self.append_entries(req).await {
                Ok(r) => Ok(r.into()),
                Err(RaftError::APIError(e)) => Ok(e.into()),
                Err(RaftError::Fatal(e)) => Err(e),
            },
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

    pub(crate) async fn handle_p2p_request(
        &self,
        from: C::NodeId,
        req: P2pRequest<C>,
    ) -> anyhow::Result<P2pResponse<C>> {
        let from_current_voter = self.is_voter(&from).await?;

        // TODO: avoid clone?
        let res = match req.clone() {
            P2pRequest::Propose(data) => {
                if !from_current_voter {
                    return Ok(P2pResponse::P2pError(P2pError::NotVoter));
                }
                match self.write_data(data).await {
                    Ok(log_id) => P2pResponse::Committed { log_id },
                    Err(e) => P2pResponse::RaftError(e),
                }
            }
            P2pRequest::Join => {
                match self
                    .change_membership(
                        ChangeMembers::AddVoters(
                            btreemap![from.clone() => (self.nodemap)(from.clone())],
                        ),
                        true,
                    )
                    .await
                {
                    Ok(_) => P2pResponse::Ok,
                    Err(e) => P2pResponse::RaftError(e),
                }
            }
            P2pRequest::Leave => {
                match self
                    .change_membership(ChangeMembers::RemoveVoters([from.clone()].into()), true)
                    .await
                {
                    Ok(_) => P2pResponse::Ok,
                    Err(e) => P2pResponse::RaftError(e),
                }
            }
        };

        Ok(res)
    }

    pub async fn shutdown(&self) -> anyhow::Result<()> {
        self.cancel.cancel();
        self.raft
            .shutdown()
            .await
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(())
    }

    async fn chore_loop(self) {
        let source = self.id.clone();
        let raft = self.clone();

        let mut interval = tokio::time::interval(self.config.join_interval);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;

            if let Some(leader) = raft.current_leader().await {
                let is_leader = leader == source;

                let is_voter = if let Ok(l) = raft.is_voter(&source).await {
                    l
                } else {
                    continue;
                };

                if is_leader || is_voter {
                    continue;
                }

                // if there is a leader and I'm not a voter, ask to rejoin the cluster
                match raft.network.send_rpc(leader, P2pRequest::Join).await {
                    Ok(P2pResponse::Ok) => {}
                    r => {
                        tracing::error!("failed to send join request to leader: {r:?}");
                    }
                }
            }
        }
    }

    async fn signal_loop(self, signal_tx: SignalSender<C>) {
        let mut index = 0;
        loop {
            match self
                .raft
                .wait(None)
                .log_index_at_least(Some(index), "log has grown")
                .await
            {
                Ok(metrics) => match self.read_log_entries(index).await {
                    Ok(entries) => {
                        debug_assert!(entries.len() > 0);
                        if let Some(last) = entries.last() {
                            debug_assert_eq!(metrics.last_log_index, Some(last.log_id.index));
                            index = last.log_id.index + 1;
                        }
                        for entry in entries {
                            if let Some(event) = self.event_from_op(entry) {
                                if let Err(e) = signal_tx.send((self.id.clone(), event)).await {
                                    tracing::warn!("failed to send RaftEvent signal: {e:?}");
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("failed to read log data: {e:?}");
                    }
                },
                Err(WaitError::ShuttingDown) => {
                    break;
                }
                Err(WaitError::Timeout(_, _)) => {
                    unreachable!()
                }
            }
        }
    }

    fn event_from_op(&self, e: C::Entry) -> Option<RaftEvent<C>> {
        match &e.payload {
            EntryPayload::Normal(data) => Some(RaftEvent::EntryCommitted {
                log_id: e.log_id.clone(),
                data: data.clone(),
            }),
            EntryPayload::Membership(m) => {
                // only send membership signals if the membership is not in a joint config
                let enabled = self.config.unstable_membership_signals;
                let stable_config = m.get_joint_config().len() <= 1;
                (enabled && stable_config).then(|| RaftEvent::MembershipChanged {
                    log_id: e.log_id.clone(),
                    members: m.voter_ids().collect::<BTreeSet<_>>(),
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::D: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::D: serde::de::DeserializeOwned"
))]
pub struct LogOp<C: TypeCfg> {
    pub log_id: LogIdOf<C>,
    pub op: C::D,
}

#[cfg(feature = "testing")]
impl<C: TypeCfg, N: P2pNetwork<C>> P2pRaft<C, N>
where
    C: TypeCfg<Entry = Entry<C>>,
{
    #[allow(unused_variables)]
    pub async fn debug_line(&self, verbose: bool) -> String {
        use itertools::Itertools;
        let t = self.tracker.lock().await;
        let peers = t.responsive_peers(self.config.responsive_interval);
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

        if verbose {
            let log = self.read_log_ops(0).await;
            let snapshot = self.raft.get_snapshot().await.unwrap().map(|s| s.snapshot);

            [
                format!("... "),
                format!("{}", self.id),
                format!("<{:?}>", self.current_leader().await),
                format!("members {:?}", members),
                format!("sees {:?}", peers),
                format!("snapshot {:?}", snapshot),
                format!("log {:?}", log),
            ]
            .into_iter()
            .join(" ")
        } else {
            [
                format!("... "),
                format!("{}", self.id),
                format!("<{:?}>", self.current_leader().await),
                format!("members {:?}", members),
                format!("sees {:?}", peers),
                // format!("snapshot {:?}", snapshot),
                // format!("log {:?}", log),
            ]
            .into_iter()
            .join(" ")
        }
    }
}
