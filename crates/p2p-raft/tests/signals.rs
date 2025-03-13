use std::{collections::BTreeMap, time::Duration};

use itertools::Itertools;
use p2p_raft::{testing::*, Config};

/// EntryCommitted signals are eventually emitted by all members,
/// and there are no duplicates.
#[tokio::test(flavor = "multi_thread")]
async fn receive_signals() {
    const NUM_PEERS: u64 = 4;
    const NUM_ENTRIES: u64 = 6;

    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let mut router = Router::new(Config::testing(50), Some(tx));
    let rafts = router.add_nodes(0..NUM_PEERS).await;
    router.natural_startup(Duration::from_millis(10)).await;

    let leader = await_any_leader(&rafts).await as usize;

    for i in 0..NUM_ENTRIES / 2 {
        rafts[leader].write_linearizable(i).await.unwrap();
    }

    router.create_partitions([0..=2, 3..=5]).await;
    await_partition_stability(&rafts[0..=2]).await;

    let leader = await_any_leader(&rafts).await as usize;

    for i in 0..NUM_ENTRIES / 2 {
        rafts[leader].write_linearizable(i).await.unwrap();
    }

    router.create_partitions([0..NUM_PEERS]).await;
    await_partition_stability(&rafts).await;

    let mut signals = BTreeMap::new();

    while let Ok((raft_id, event)) = rx.try_recv() {
        println!("event: {raft_id} {:?}", event);
        let e = signals.entry(event).or_insert(0);
        *e += 1;
    }

    assert_eq!(signals.len(), NUM_ENTRIES as usize);
    assert_eq!(
        signals.values().copied().collect_vec(),
        vec![NUM_PEERS - 1; NUM_ENTRIES as usize]
    );
}
