use futures::FutureExt;
use maplit::btreeset;
use memstore::{StateMachineStore, TypeConfig};
use openraft::{EntryPayload, Snapshot, SnapshotMeta, Vote};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

use p2p_raft::testing::Router;

pub type Dinghy = p2p_raft::Dinghy<memstore::TypeConfig>;

async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // tracing_subscriber::fmt::fmt()
    //     .with_max_level(tracing::Level::ERROR)
    //     .init();

    const N: u64 = 5;
    let all_ids = (0..N).collect::<BTreeSet<_>>();
    let mut router = Router::default();
    let rafts = router.add_nodes(all_ids.clone()).await;

    // router
    //     .create_partitions(vec![vec![0], vec![1], vec![2], vec![3], vec![4]])
    //     .await;

    println!("router created.");

    for (_, raft) in rafts.iter().enumerate() {
        // let peers = all_ids.iter().take(i + 1).cloned().collect::<BTreeSet<_>>();
        // dbg!(&peers);
        raft.initialize(all_ids.clone()).await.unwrap();
        println!("initialized {}.", raft.id);
        sleep(100).await;
    }

    spawn_info_loop(rafts.clone());

    sleep(5000).await;

    let leader = await_single_leader(&rafts, None).await as usize;
    println!("leader: {leader}");

    rafts[leader].ensure_linearizable().await.unwrap();
    rafts[leader].client_write(0).await.unwrap();
    println!("wrote data.");
    rafts[leader].ensure_linearizable().await.unwrap();
    rafts[leader].client_write(1).await.unwrap();
    println!("wrote data.");
    rafts[leader].ensure_linearizable().await.unwrap();
    rafts[leader].client_write(2).await.unwrap();
    println!("wrote data.");

    for n in [0, 1] {
        router.create_partitions(vec![vec![n]]).await;
        sleep(5_000).await;
    }

    let leader = await_single_leader(&rafts[3..], Some(leader as u64)).await;
    println!("leader: {leader}");

    replace_snapshot(&rafts[leader as usize], vec![5, 4, 3, 2, 1]).await;

    sleep(10_000).await;
}

/// NOTE: only run this as leader!
/// XXX: really this is just a workaround for when it's not feasible to implement
///      merging in the state machine, when that logic needs to be in the front end
///      and the merged snapshot is forced by the leader.
async fn replace_snapshot(raft: &Dinghy, data: Vec<memstore::Request>) {
    let smd = {
        let mut sm = raft
            .raft
            .with_state_machine(|s: &mut Arc<StateMachineStore<TypeConfig>>| {
                async move { s.state_machine.lock().unwrap().clone() }.boxed()
            })
            .await
            .unwrap()
            .unwrap();
        sm.data = data;
        sm
    };

    let term = smd
        .last_applied
        .map(|l| l.committed_leader_id().term)
        .unwrap_or(0);

    let snapshot = Snapshot {
        snapshot: Box::new(smd.clone()),
        meta: SnapshotMeta {
            last_log_id: smd.last_applied,
            last_membership: smd.last_membership,
            snapshot_id: nanoid::nanoid!(),
        },
    };

    raft.install_full_snapshot(Vote::new(term, raft.id), snapshot)
        .await
        .unwrap();
}

fn spawn_info_loop(mut rafts: Vec<Dinghy>) {
    use openraft::RaftLogReader;
    use openraft::storage::RaftLogStorage;

    tokio::spawn({
        const POLL_INTERVAL: Duration = Duration::from_millis(1000);

        async move {
            loop {
                for r in rafts.iter_mut() {
                    let t = r.tracker.lock().await;
                    let peers = t.responsive_peers(p2p_raft::RESPONSIVE_INTERVAL);
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
                    let log = r
                        .store
                        .get_log_reader()
                        .await
                        .try_get_log_entries(..)
                        .await
                        .unwrap()
                        .into_iter()
                        .filter_map(|e| match e.payload {
                            EntryPayload::Normal(n) => Some(n),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
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
    let start = std::time::Instant::now();
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

async fn await_single_leader(rafts: &[Dinghy], previous: Option<u64>) -> u64 {
    loop {
        let leaders = await_leaders(rafts, previous.map(|p| btreeset![p])).await;
        if leaders.len() == 1 {
            return leaders.into_iter().next().unwrap();
        } else if leaders.len() > 1 {
            println!("multiple leaders: {leaders:?}");
        }
        sleep(250).await;
    }
}
