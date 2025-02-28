use maplit::btreeset;
use std::{collections::BTreeSet, time::Duration};

use p2p_raft::testing::Router;

pub type Request = String;
pub type Response = ();
pub type StateMachineData = Vec<Request>;

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

    // router.create_partitions(vec![vec![0, 1, 2, 3, 4]]).await;

    tokio::spawn({
        let rafts = rafts.clone();
        const POLL_INTERVAL: Duration = Duration::from_millis(1000);

        async move {
            loop {
                for r in rafts.iter() {
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
                    println!(
                        "{}  <{:?}>  members {:?}   sees {:?} ",
                        r.id,
                        r.current_leader().await,
                        members,
                        peers
                    );
                }
                println!("--------------------------------------------------------");
                sleep(POLL_INTERVAL.as_millis() as u64).await;
            }
        }
    });

    sleep(5000).await;

    let leader = await_single_leader(&rafts, None).await as usize;
    println!("leader: {leader}");

    dbg!();
    rafts[leader].client_write(0).await.unwrap();
    dbg!();

    for n in [0, 1] {
        router.create_partitions(vec![vec![0]]).await;
        sleep(5_000).await;
    }

    let leaders = await_single_leader(&rafts[3..], Some(leader as u64)).await;
    // let leaders = await_leaders(&rafts, None).await;
    println!("leader: {leaders:?}");

    // js.abort_all();
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
