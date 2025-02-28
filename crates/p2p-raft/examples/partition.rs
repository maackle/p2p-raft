use maplit::btreeset;
use std::{collections::BTreeSet, time::Duration};

use p2p_raft::testing::Router;

pub type Request = String;
pub type Response = ();
pub type StateMachineData = Vec<Request>;

pub type Dinghy = p2p_raft::Dinghy<memstore::TypeConfig>;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    tracing_subscriber::fmt::fmt()
        .with_max_level(tracing::Level::WARN)
        .init();

    const N: u64 = 5;
    let all_ids = (0..N).collect::<BTreeSet<_>>();
    let mut router = Router::default();
    let rafts = router.add_nodes(0..N).await;

    for raft in rafts.iter() {
        raft.initialize(all_ids.clone()).await.unwrap();
    }

    let leader = await_single_leader(&rafts, None).await as usize;
    println!("leader: {leader}");

    rafts[leader].client_write(0).await.unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let split = 3; //leader as u64;
    println!("----------------- PARTITIONED < {split} ---------------------");
    router.partitions(vec![0..split, split..N]).await;

    let leaders = await_leaders(&rafts, Some(btreeset![leader as u64])).await;
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
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
            let here = dinghy
                .tracker
                .lock()
                .await
                .responsive_peers(Duration::from_millis(250));
            println!("{} sees {:?}", dinghy.id(), here);
        }
        if let Some(previous) = previous.as_ref() {
            if leaders == *previous {
                tokio::time::sleep(Duration::from_millis(250)).await;
                continue;
            }
        }
        println!("time: {:?}", start.elapsed());
        return leaders;
    }
}

async fn await_single_leader(rafts: &[Dinghy], previous: Option<u64>) -> u64 {
    loop {
        let leaders = await_leaders(rafts, previous.map(|p| btreeset![p])).await;
        if leaders.len() == 1 {
            return leaders.into_iter().next().unwrap();
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}
