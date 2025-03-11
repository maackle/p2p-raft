use std::{collections::BTreeMap, time::Duration};

use itertools::Itertools;
use p2p_raft::{testing::*, DinghyConfig};

#[tokio::test(flavor = "multi_thread")]
async fn receive_signals() {
    const NUM_PEERS: u64 = 4;
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let mut router = Router::new(DinghyConfig::testing(50), Some(tx));
    let rafts = router.add_nodes(0..NUM_PEERS).await;
    router.natural_startup(Duration::from_millis(100)).await;

    let leader = await_any_leader(&rafts).await as usize;

    rafts[leader].write_linearizable(0).await.unwrap();
    rafts[leader].write_linearizable(1).await.unwrap();
    rafts[leader].write_linearizable(2).await.unwrap();

    sleep(100).await;

    let mut signals = BTreeMap::new();

    while let Ok(event) = rx.try_recv() {
        println!("event: {:?}", event);
        let e = signals.entry(event).or_insert(0);
        *e += 1;
    }

    assert_eq!(signals.len(), 4);
    assert_eq!(
        signals.values().copied().collect_vec(),
        vec![NUM_PEERS - 1; 4]
    );
}
