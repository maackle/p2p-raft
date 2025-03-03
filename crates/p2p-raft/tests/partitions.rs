use futures::FutureExt;
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

    let leader = await_single_leader(&rafts[3..], None).await as usize;

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
}
