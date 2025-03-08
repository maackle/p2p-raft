use std::{collections::BTreeSet, time::Duration};

use itertools::Itertools;
use openraft::ServerState;

use crate::{config::DinghyConfig, network::P2pNetwork};

use super::*;

type TestDinghy = crate::Dinghy<super::TypeConfig, RouterNode>;

pub async fn initialized_router(num_peers: u64, config: DinghyConfig) -> (Router, Vec<TestDinghy>) {
    let all_ids = (0..num_peers).collect::<BTreeSet<_>>();
    let mut router = Router::new(config);
    let rafts = router.add_nodes(all_ids.clone()).await;

    println!("router created.");

    for (_, raft) in rafts.iter().enumerate() {
        let _ = tokio::spawn(raft.clone().chore_loop());
        raft.initialize(all_ids.clone()).await.unwrap();
        println!("initialized {}.", raft.id);
    }

    (router, rafts)
}

#[allow(warnings)]
pub fn spawn_info_loop<C, N>(mut rafts: Vec<crate::Dinghy<C, N>>, poll_interval_ms: u64)
where
    C: crate::TypeCfg<
        Entry = openraft::Entry<C>,
        SnapshotData = p2p_raft_memstore::StateMachineData<C>,
    >,
    N: P2pNetwork<C>,
{
    tokio::spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));
        async move {
            loop {
                interval.tick().await;
                println!();
                println!("........................................................");
                for r in rafts.iter_mut() {
                    println!("{}", r.debug_line().await);
                }
                println!("........................................................");
                println!();
            }
        }
    });
}

pub async fn await_any_leader_t<C, N>(
    dinghies: &[crate::Dinghy<C, N>],
    timeout: Option<Duration>,
) -> anyhow::Result<C::NodeId>
where
    C: crate::TypeCfg<
        Entry = openraft::Entry<C>,
        SnapshotData = p2p_raft_memstore::StateMachineData<C>,
    >,
    N: P2pNetwork<C>,
{
    let start = std::time::Instant::now();
    let ids = dinghies.iter().map(|r| r.id.clone()).collect_vec();
    println!("awaiting any leader for {ids:?}");
    let futs = dinghies.iter().map(|r| {
        Box::pin(async move {
            r.raft
                .wait(timeout)
                .state(ServerState::Leader, "await_state")
                .await
        })
    });

    let (res, _idx, _) = futures::future::select_all(futs).await;
    let leader = res?.id;
    println!("found new leader {leader} in {:?}", start.elapsed());
    Ok(leader)
}

pub async fn await_any_leader<C, N>(dinghies: &[crate::Dinghy<C, N>]) -> C::NodeId
where
    C: crate::TypeCfg<
        Entry = openraft::Entry<C>,
        SnapshotData = p2p_raft_memstore::StateMachineData<C>,
    >,
    N: P2pNetwork<C>,
{
    let start = std::time::Instant::now();
    let ids = dinghies.iter().map(|r| r.id.clone()).collect_vec();
    println!("awaiting any leader for {ids:?}");
    let futs = dinghies.iter().map(|r| {
        Box::pin(async move {
            r.raft
                .wait(None)
                .state(ServerState::Leader, "await leader")
                .await
        })
    });

    let (res, _idx, _) = futures::future::select_all(futs).await;
    let leader = res.unwrap().id;
    let election_time = start.elapsed();

    futures::future::join_all(dinghies.iter().map(|r| {
        let leader = leader.clone();
        async move {
            if r.id != leader {
                r.wait(None)
                    .current_leader(leader, "await consensus")
                    .await
                    .unwrap();
            }
        }
    }))
    .await;
    let consensus_time = start.elapsed();

    println!(
        "elected new leader {leader} in {:?}, full consensus in {:?}",
        election_time, consensus_time
    );
    leader
}

pub async fn await_partition_stability<C, N>(dinghies: &[crate::Dinghy<C, N>])
where
    C: crate::TypeCfg<
        Entry = openraft::Entry<C>,
        SnapshotData = p2p_raft_memstore::StateMachineData<C>,
    >,
    N: P2pNetwork<C>,
{
    let start = std::time::Instant::now();
    let ids = dinghies.iter().map(|r| r.id.clone()).collect_vec();

    println!("~~ awaiting stability for partition {ids:?}",);

    futures::future::join_all(dinghies.iter().map(|r| {
        let r = r.clone();
        let ids = ids.clone();
        async move { r.wait(None).voter_ids(ids, "partition stability").await }
    }))
    .await;

    println!(
        "~~ partition {ids:?} stabilized in {:?} ~~~~~~~~~~~~~~~~~~~~~~",
        start.elapsed()
    );

    // somehow this little sleep is needed
    sleep(100).await;
}

pub async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
