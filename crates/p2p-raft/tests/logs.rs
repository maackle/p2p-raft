use std::{collections::BTreeMap, time::Duration};

use itertools::Itertools;
use p2p_raft::{testing::*, Config};

#[tokio::test(flavor = "multi_thread")]
async fn test_prev_op_log_id() {
    const N: u64 = 4;
    let mut router = Router::new(Config::testing(50), None);
    let mut rafts: BTreeMap<NodeId, TestRaft> = router
        .add_nodes(0..N)
        .await
        .into_iter()
        .map(|r| (r.id, r))
        .collect();
    router.natural_startup(Duration::from_millis(10)).await;
    let leader = await_any_leader(rafts.values()).await;

    let commit0 = rafts[&leader].write_linearizable(0).await.unwrap();
    assert_eq!(commit0.prev_op_log_id, None);

    let commit1 = rafts[&leader].write_linearizable(1).await.unwrap();
    assert_eq!(commit1.prev_op_log_id, Some(commit0.log_id));

    rafts.remove(&leader);
    let ids = rafts.keys().copied().collect_vec();
    router.create_partitions([vec![leader], ids]).await;
    await_partition_stability(rafts.values()).await;
    let leader2 = await_any_leader(rafts.values()).await;
    assert_ne!(leader, leader2);

    let commit2 = rafts[&leader2].write_linearizable(2).await.unwrap();
    assert_eq!(commit2.prev_op_log_id, Some(commit1.log_id));
}
