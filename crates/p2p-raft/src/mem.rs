use std::sync::Arc;

use openraft::{alias::NodeIdOf, storage::RaftStateMachine};
use p2p_raft_memstore::ArcStateMachineStore;

use crate::{config::Config, network::P2pNetwork, signal::SignalSender};

use super::*;

impl<C, N: P2pNetwork<C>> P2pRaft<C, N>
where
    C: TypeCfg,
    ArcStateMachineStore<C>: RaftStateMachine<C>,
{
    /// Create a new raft instance with in-memory storage and a trivial state machine.
    pub async fn create_in_memory(
        node_id: NodeIdOf<C>,
        config: impl Into<Arc<Config>>,
        network: N,
        signal_tx: Option<SignalSender<C>>,
        nodemap: impl Fn(C::NodeId) -> C::Node + Send + Sync + 'static,
    ) -> anyhow::Result<Self> {
        let config = config.into();

        let log_store = LogStore::default();

        let state_machine_store =
            ArcStateMachineStore::from(Arc::new(p2p_raft_memstore::StateMachineStore::default()));

        let raft = openraft::Raft::new(
            node_id.clone(),
            config.raft_config.clone().into(),
            network.clone(),
            log_store.clone(),
            state_machine_store.clone(),
        )
        .await?;

        Ok(P2pRaft {
            raft,
            id: node_id,
            store: log_store,
            tracker: PeerTrackerHandle::new(),
            network,
            config,
            signal_tx,
            nodemap: Arc::new(nodemap),
        })
    }

    // /// NOTE: only run this as leader!
    // /// XXX: really this is just a workaround for when it's not feasible to implement
    // ///      merging in the state machine, when that logic needs to be in the front end
    // ///      and the merged snapshot is forced by the leader.
    // #[deprecated = "this does not work!"]
    // pub async fn replace_snapshot(&self, data: Vec<p2p_raft_memstore::Request>) {
    //     use openraft::storage::RaftStateMachine;

    //     let smd = {
    //         let mut sm = self
    //             .raft
    //             .with_state_machine(|s: &mut Arc<StateMachineStore<C>>| {
    //                 async move { s.state_machine.lock().unwrap().clone() }.boxed()
    //             })
    //             .await
    //             .unwrap()
    //             .unwrap();

    //         sm.data = data;
    //         // sm.last_applied.map(|mut l| {
    //         //     l.index += 1;
    //         //     l
    //         // });
    //         sm
    //     };

    //     let snapshot = Box::new(smd.clone());

    //     let snapshot_id = self
    //         .raft
    //         .with_raft_state(|s| s.snapshot_meta.snapshot_id.clone())
    //         .await
    //         .unwrap();

    //     let meta = SnapshotMeta {
    //         last_log_id: smd.last_applied,
    //         last_membership: smd.last_membership,
    //         snapshot_id,
    //         // snapshot_id: nanoid::nanoid!(),
    //     };

    //     self.with_state_machine(move |s: &mut Arc<StateMachineStore<C>>| {
    //         async move {
    //             s.clone()
    //                 .install_snapshot(&meta.clone(), snapshot)
    //                 .await
    //                 .unwrap();
    //             // s.build_snapshot().await.unwrap();
    //         }
    //         .boxed()
    //     })
    //     .await
    //     .unwrap()
    //     .unwrap();

    //     let trigger = self.raft.trigger();
    //     trigger
    //         .purge_log(smd.last_applied.map(|l| l.index).unwrap_or_default())
    //         .await
    //         .unwrap();
    //     trigger.snapshot().await.unwrap();
    //     trigger.heartbeat().await.unwrap();

    //     // raft.install_full_snapshot(Vote::new(term, raft.id), snapshot)
    //     //     .await
    //     //     .unwrap();
    // }
}
