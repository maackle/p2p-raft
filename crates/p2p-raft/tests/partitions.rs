use std::time::Duration;

use p2p_raft::testing::*;

#[tokio::test(flavor = "multi_thread")]
async fn shrink_and_grow_and_shrink() {
    // tracing_subscriber::fmt::fmt()
    //     .with_max_level(tracing::Level::ERROR)
    //     .init();

    const NUM_PEERS: u64 = 5;

    let (mut router, rafts) = initialized_router(NUM_PEERS).await;

    // spawn_info_loop(rafts.clone(), 5000);

    let leader = await_any_leader(&rafts).await as usize;

    rafts[leader].write_linearizable(0).await.unwrap();
    rafts[leader].write_linearizable(1).await.unwrap();
    rafts[leader].write_linearizable(2).await.unwrap();
    println!("wrote data.");

    // - now gradually whittle down the cluster until only 2 nodes are left

    router.create_partitions([vec![0, 1], vec![2, 3, 4]]).await;
    await_partition_stability(&rafts[2..]).await;

    router.create_partitions([vec![2], vec![3, 4]]).await;
    await_partition_stability(&rafts[3..]).await;

    let leader = await_any_leader(&rafts[3..]).await as usize;

    rafts[leader].write_linearizable(3).await.unwrap();
    rafts[leader].write_linearizable(4).await.unwrap();
    rafts[leader].write_linearizable(5).await.unwrap();
    println!("wrote data in remaining raft.");

    // - heal the cluster, bringing all nodes back into the same partition

    router.create_partitions([0..=4]).await;
    await_partition_stability(&rafts[0..=4]).await;

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

    // - one of the originally partitioned nodes may become leader again

    router.create_partitions([0..=3]).await;
    await_partition_stability(&rafts[0..=3]).await;
    router.create_partitions([0..=2]).await;
    await_partition_stability(&rafts[0..=2]).await;

    // await_any_leader_t(&rafts[0..=2], Some(Duration::from_secs(3)))
    await_any_leader_t(&rafts[0..=2], Some(Duration::from_secs(30)))
        .await
        .unwrap();
}
