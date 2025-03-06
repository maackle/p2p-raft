use std::{collections::BTreeSet, time::Duration};

use itertools::Itertools;
use openraft::ServerState;

use crate::config::DinghyConfig;

use super::*;

pub type Dinghy = crate::Dinghy<super::TypeConfig, RouterNode>;

pub async fn initialized_router(num_peers: u64, config: DinghyConfig) -> (Router, Vec<Dinghy>) {
    let all_ids = (0..num_peers).collect::<BTreeSet<_>>();
    let mut router = Router::new(config);
    let rafts = router.add_nodes(all_ids.clone()).await;

    println!("router created.");

    for (_, raft) in rafts.iter().enumerate() {
        let _ = raft.spawn_chore_loop();
        raft.initialize(all_ids.clone()).await.unwrap();
        println!("initialized {}.", raft.id);
    }

    (router, rafts)
}

#[allow(warnings)]
pub fn spawn_info_loop(mut rafts: Vec<Dinghy>, poll_interval_ms: u64) {
    tokio::spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));
        async move {
            loop {
                interval.tick().await;
                println!();
                println!("........................................................");
                for r in rafts.iter_mut() {
                    let t = r.tracker.lock().await;
                    let peers = t.responsive_peers(r.config.p2p_config.responsive_interval);
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

                    let lines = [
                        format!("... "),
                        format!("{}", r.id),
                        format!("<{:?}>", r.current_leader().await),
                        format!("members {:?}", members),
                        format!("sees {:?}", peers),
                        // format!("snapshot {:?}", snapshot),
                        // format!("log {:?}", log),
                    ];

                    println!("{}", lines.into_iter().join(" "))
                }
                println!("........................................................");
                println!();
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
                .state(ServerState::Leader, "await leader")
                .await
        })
    });

    let (res, _idx, _) = futures::future::select_all(futs).await;
    let leader = res.unwrap().id;
    let election_time = start.elapsed();

    futures::future::join_all(dinghies.iter().map(|r| async move {
        if r.id != leader {
            r.wait(None)
                .current_leader(leader, "await consensus")
                .await
                .unwrap();
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

pub async fn await_partition_stability(dinghies: &[Dinghy]) {
    let start = std::time::Instant::now();
    let ids = dinghies.iter().map(|r| r.id).collect_vec();

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
