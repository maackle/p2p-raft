use std::{collections::BTreeSet, time::Duration};

use itertools::Itertools;
use maplit::btreeset;
use openraft::ServerState;

use super::*;

pub type Dinghy = crate::Dinghy<memstore::TypeConfig>;

pub async fn initialized_router(num_peers: u64) -> (Router<memstore::TypeConfig>, Vec<Dinghy>) {
    let all_ids = (0..num_peers).collect::<BTreeSet<_>>();
    let mut router = Router::default();
    let rafts = router.add_nodes(all_ids.clone()).await;

    println!("router created.");

    for (_, raft) in rafts.iter().enumerate() {
        raft.initialize(all_ids.clone()).await.unwrap();
        println!("initialized {}.", raft.id);
    }

    (router, rafts)
}

pub fn spawn_info_loop(mut rafts: Vec<Dinghy>, poll_interval_ms: u64) {
    tokio::spawn({
        async move {
            loop {
                for r in rafts.iter_mut() {
                    let t = r.tracker.lock().await;
                    let peers = t.responsive_peers(crate::RESPONSIVE_INTERVAL);
                    let members = r
                        .raft
                        .with_raft_state(|s| {
                            s.membership_state
                                .committed()
                                .voter_ids()
                                .collect::<BTreeSet<_>>()
                        })
                        .await
                        .unwrap();
                    let log = r.read_log_data().await;
                    let snapshot = r
                        .raft
                        .get_snapshot()
                        .await
                        .unwrap()
                        .map(|s| s.snapshot.data);
                    println!(
                        "  {}  <{:?}>  members {:?}   sees {:?}  snapshot {:?}   log {:?}",
                        r.id,
                        r.current_leader().await,
                        members,
                        peers,
                        snapshot,
                        log
                    );
                }
                println!("  ........................................................");
                sleep(poll_interval_ms).await;
            }
        }
    });
}

pub async fn await_any_leader_t(
    dinghies: &[Dinghy],
    timeout: Option<Duration>,
) -> anyhow::Result<u64> {
    let start = std::time::Instant::now();
    let ids = dinghies.iter().map(|r| r.id).collect_vec();
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

pub async fn await_any_leader(dinghies: &[Dinghy]) -> u64 {
    let start = std::time::Instant::now();
    let ids = dinghies.iter().map(|r| r.id).collect_vec();
    println!("awaiting any leader for {ids:?}");
    let futs = dinghies.iter().map(|r| {
        Box::pin(async move {
            r.raft
                .wait(None)
                .state(ServerState::Leader, "await_state")
                .await
        })
    });

    let (res, _idx, _) = futures::future::select_all(futs).await;
    let leader = res.unwrap().id;
    println!("found new leader {leader} in {:?}", start.elapsed());
    leader
}

pub async fn await_partition_stability(dinghies: &[Dinghy]) {
    let start = std::time::Instant::now();
    futures::future::join_all(dinghies.iter().map(|r| {
        let r = r.clone();
        async move { r.wait(None).voter_ids(vec![2, 3, 4], "partition 0 1").await }
    }))
    .await;
    println!(
        "awaiting partition {:?} stabilized in {:?}",
        dinghies.iter().map(|r| r.id).collect_vec(),
        start.elapsed()
    );
    sleep(3000).await;
}

pub async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
