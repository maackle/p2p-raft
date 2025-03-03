use std::{collections::BTreeSet, time::Duration};

use itertools::Itertools;
use maplit::btreeset;

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

pub fn spawn_info_loop(mut rafts: Vec<Dinghy>) {
    tokio::spawn({
        const POLL_INTERVAL: Duration = Duration::from_millis(1000);

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
                sleep(POLL_INTERVAL.as_millis() as u64).await;
            }
        }
    });
}

async fn await_leaders(dinghies: &[Dinghy], previous: Option<BTreeSet<u64>>) -> BTreeSet<u64> {
    loop {
        let mut leaders = BTreeSet::new();
        for dinghy in dinghies.iter() {
            if let Some(leader) = dinghy.current_leader().await {
                leaders.insert(leader);
            } else {
                sleep(250).await;
                continue;
            }
        }
        if let Some(previous) = previous.as_ref() {
            if leaders == *previous {
                sleep(250).await;
                continue;
            }
        }
        return leaders;
    }
}

pub async fn await_single_leader(rafts: &[Dinghy], previous: Option<u64>) -> u64 {
    let start = std::time::Instant::now();
    let ids = rafts.iter().map(|r| r.id).collect_vec();
    if let Some(previous) = previous {
        println!("awaiting single leader != {previous} for {ids:?}");
    } else {
        println!("awaiting single leader for {ids:?}");
    }
    loop {
        let leaders = await_leaders(rafts, previous.map(|p| btreeset![p])).await;
        if leaders.len() == 1 {
            let leader = leaders.into_iter().next().unwrap();
            println!("found new leader {leader} in {:?}", start.elapsed());
            return leader;
        } else if leaders.len() > 1 {
            println!("multiple leaders: {leaders:?}");
        }
        sleep(250).await;
    }
}

pub async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}
