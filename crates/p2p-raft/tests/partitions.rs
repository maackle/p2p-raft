use futures::{FutureExt, future::join_all};
use itertools::Itertools;
use maplit::btreeset;
use memstore::{StateMachineStore, TypeConfig};
use openraft::{EntryPayload, Snapshot, SnapshotMeta, Vote};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

use p2p_raft::{RESPONSIVE_INTERVAL, testing::*};

const PARTITION_DELAY: u64 = RESPONSIVE_INTERVAL.as_millis() as u64 * 3;

#[tokio::test(flavor = "multi_thread")]
async fn shrink_and_grow() {
    // tracing_subscriber::fmt::fmt()
    //     .with_max_level(tracing::Level::ERROR)
    //     .init();

    const NUM_PEERS: u64 = 5;

    let (mut router, rafts) = initialized_router(NUM_PEERS).await;
    spawn_info_loop(rafts.clone());

    let leader = await_any_leader(&rafts).await as usize;

    rafts[leader].write_linearizable(0).await.unwrap();
    rafts[leader].write_linearizable(1).await.unwrap();
    rafts[leader].write_linearizable(2).await.unwrap();
    println!("wrote data.");

    // - now gradually whittle down the cluster until only 2 nodes are left

    router.create_partitions(vec![vec![0, 1]]).await;

    join_all(rafts[3..].iter().map(|r| {
        let r = r.clone();
        async move { r.wait(None).voter_ids(vec![2, 3, 4], "partition 0 1").await }
    }))
    .await;

    router.create_partitions(vec![vec![2]]).await;

    join_all(rafts[3..].iter().map(|r| {
        let r = r.clone();
        async move { r.wait(None).voter_ids(vec![3, 4], "partition 0 1").await }
    }))
    .await;

    let leader = await_any_leader(&rafts[3..]).await as usize;
    sleep(100).await;

    rafts[leader].write_linearizable(3).await.unwrap();
    rafts[leader].write_linearizable(4).await.unwrap();
    rafts[leader].write_linearizable(5).await.unwrap();
    println!("wrote data in remaining raft.");

    // - heal the cluster, bringing all nodes back into the same partition

    router.create_partitions(vec![vec![0, 1, 2, 3, 4]]).await;

    rafts[0]
        .wait(None)
        .current_leader(leader as u64, "heal")
        .await
        .unwrap();

    // TODO: re-add original nodes as voters when they are responsive again.

    // - one of the originally partitioned nodes will eventually have the full log
    {
        let log = rafts[0].read_log_data().await.unwrap();
        println!("log: {log:?}");
        assert_eq!(log, vec![0, 1, 2, 3, 4, 5]);
    }
}
