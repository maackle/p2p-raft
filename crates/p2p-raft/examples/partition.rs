use futures::FutureExt;
use maplit::btreeset;
use memstore::{StateMachineStore, TypeConfig};
use openraft::{EntryPayload, Snapshot, SnapshotMeta, Vote};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

use p2p_raft::{RESPONSIVE_INTERVAL, testing::Router};

pub type Dinghy = p2p_raft::Dinghy<memstore::TypeConfig>;

async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // tracing_subscriber::fmt::fmt()
    //     .with_max_level(tracing::Level::ERROR)
    //     .init();

    const NUM_PEERS: u64 = 5;
    const PARTITION_DELAY: u64 = RESPONSIVE_INTERVAL.as_millis() as u64 * 3;

    let all_ids = (0..NUM_PEERS).collect::<BTreeSet<_>>();
    let mut router = Router::default();
    let rafts = router.add_nodes(all_ids.clone()).await;

    println!("router created.");

    for (_, raft) in rafts.iter().enumerate() {
        raft.initialize(all_ids.clone()).await.unwrap();
        println!("initialized {}.", raft.id);
        sleep(100).await;
    }

    spawn_info_loop(rafts.clone());

    let leader = await_single_leader(&rafts, None).await as usize;

    rafts[leader].write_linearizable(0).await.unwrap();
    rafts[leader].write_linearizable(1).await.unwrap();
    rafts[leader].write_linearizable(2).await.unwrap();
    println!("wrote data.");

    // now gradually whittle down the cluster until only 2 nodes are left
    router.create_partitions(vec![vec![0, 1]]).await;
    sleep(PARTITION_DELAY).await;
    router.create_partitions(vec![vec![2]]).await;
    sleep(PARTITION_DELAY).await;

    // rafts[0].write_linearizable(3).await.unwrap();
    // rafts[0].write_linearizable(4).await.unwrap();
    // rafts[0].write_linearizable(5).await.unwrap();
    // println!("wrote data in old raft.");

    let leader = await_single_leader(&rafts[3..], Some(leader as u64)).await as usize;

    rafts[leader].write_linearizable(3).await.unwrap();
    rafts[leader].write_linearizable(4).await.unwrap();
    rafts[leader].write_linearizable(5).await.unwrap();
    println!("wrote data in remaining raft.");

    router.create_partitions(vec![vec![0, 1, 2, 3, 4]]).await;

    sleep(PARTITION_DELAY).await;

    // TODO: re-add original nodes as voters when they are responsive again.

    // one of the partitioned nodes will eventually have the full log
    {
        let log = rafts[0].read_log_data().await.unwrap();
        println!("log: {log:?}");
        assert_eq!(log, vec![0, 1, 2, 3, 4, 5]);
    }

    replace_snapshot(&rafts[leader as usize], vec![5, 4, 3, 2, 1]).await;

    sleep(10_000).await;
}

/// NOTE: only run this as leader!
/// XXX: really this is just a workaround for when it's not feasible to implement
///      merging in the state machine, when that logic needs to be in the front end
///      and the merged snapshot is forced by the leader.
async fn replace_snapshot(raft: &Dinghy, data: Vec<memstore::Request>) {
    use openraft::storage::{RaftSnapshotBuilder, RaftStateMachine};

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
        // sm.last_applied.map(|mut l| {
        //     l.index += 1;
        //     l
        // });
        sm
    };

    let snapshot = Box::new(smd.clone());

    let meta = SnapshotMeta {
        last_log_id: smd.last_applied,
        last_membership: smd.last_membership,
        // snapshot_id,
        snapshot_id: nanoid::nanoid!(),
    };

    raft.with_state_machine(move |s: &mut Arc<StateMachineStore<TypeConfig>>| {
        async move {
            s.clone()
                .install_snapshot(&meta.clone(), snapshot)
                .await
                .unwrap();
            s.build_snapshot().await.unwrap();
        }
        .boxed()
    })
    .await
    .unwrap()
    .unwrap();

    // raft.install_full_snapshot(Vote::new(term, raft.id), snapshot)
    //     .await
    //     .unwrap();

    println!("replaced snapshot.");
}

fn spawn_info_loop(mut rafts: Vec<Dinghy>) {
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
            let leader = leaders.into_iter().next().unwrap();
            println!("new leader: {leader}");
            return leader;
        } else if leaders.len() > 1 {
            println!("multiple leaders: {leaders:?}");
        }
        sleep(250).await;
    }
}
