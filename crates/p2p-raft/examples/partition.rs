use maplit::btreeset;
use std::collections::BTreeSet;

use network_impl::router::Router;
use p2p_raft::Dinghy;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // tracing_subscriber::fmt::fmt()
    //     .with_max_level(tracing::Level::WARN)
    //     .init();

    const N: u64 = 10;
    let all_ids = (0..N).collect::<BTreeSet<_>>();
    let mut router = Router::default();
    let (rafts, _js) = router.add_nodes(0..N).await;
    let rafts = rafts
        .into_iter()
        .map(|r| Dinghy::from(r))
        .collect::<Vec<_>>();

    for raft in rafts.iter() {
        raft.initialize(all_ids.clone()).await.unwrap();
    }

    let leader = await_single_leader(&rafts, None).await as usize;
    println!("leader: {leader}");

    rafts[leader].client_write(0.into()).await.unwrap();

    let split = leader as u64 + 1;
    router.partition(vec![0..split, split..N]);
    // router.partition(vec![vec![leader as u64]]);

    // tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

    let leaders = await_leaders(&rafts, Some(btreeset![leader as u64])).await;
    // let leaders = await_leaders(&rafts, None).await;
    println!("leader: {leaders:?}");

    // js.abort_all();
}

async fn await_leaders(rafts: &[Dinghy], previous: Option<BTreeSet<u64>>) -> BTreeSet<u64> {
    let start = std::time::Instant::now();
    loop {
        let mut leaders = BTreeSet::new();
        for raft in rafts.iter() {
            if let Some(leader) = raft.current_leader().await {
                leaders.insert(leader);
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
                continue;
            }
        }
        if let Some(previous) = previous.as_ref() {
            if leaders == *previous {
                tokio::time::sleep(tokio::time::Duration::from_millis(250)).await;
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
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
