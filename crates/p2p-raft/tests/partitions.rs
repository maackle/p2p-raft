use std::time::Duration;

use p2p_raft::{testing::*, Config};

#[tokio::test(flavor = "multi_thread")]
async fn natural_startup() {
    let num_peers = 5;
    let config = Config::testing(50);

    let mut router = Router::new(config, None);
    let rafts = router.add_nodes(0..num_peers).await;

    router.natural_startup(Duration::from_millis(10)).await;

    // spawn_info_loop(rafts.clone(), 100);

    println!("router created.");

    await_partition_stability(&rafts).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn startup_race() {
    let num_peers = 3;
    let config = Config::testing(50);

    let mut router = Router::new(config, None);
    let rafts = router.add_nodes(0..num_peers).await;

    // spawn_info_loop(rafts.clone(), 100, true);

    let ids = rafts.iter().map(|r| r.id.clone()).collect::<Vec<_>>();
    for raft in rafts.iter() {
        raft.initialize([raft.id]).await.unwrap();
        raft.write_linearizable(raft.id + 10).await.unwrap();
    }

    tokio::time::sleep(Duration::from_millis(200)).await;

    for raft in rafts.iter() {
        raft.broadcast_join(ids.clone()).await.unwrap();
    }

    await_partition_stability(&rafts).await;
    let leader = await_any_leader(&rafts).await;

    rafts[leader as usize]
        .write_linearizable(100)
        .await
        .unwrap();

    for r in rafts.iter() {
        println!("{}", r.debug_line(true).await);
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn join_later() {
    const NUM_PEERS: u64 = 3;
    let mut router = Router::new(Config::testing(50), None);
    let rafts = router.add_nodes(0..NUM_PEERS).await;
    router.initialize_nodes().await;
    await_partition_stability(&rafts[..]).await;
    await_any_leader(&rafts[..]).await;

    router.add_nodes([NUM_PEERS]).await;
    let rafts = router.rafts();
    let noob = rafts[NUM_PEERS as usize].clone();

    noob.initialize(0..=NUM_PEERS).await.unwrap();
    noob.broadcast_join(0..=NUM_PEERS).await.unwrap();
    await_partition_stability(&rafts[..]).await;
    await_any_leader(&rafts[..]).await;

    // Joining is idempotent for everyone including the leader
    for raft in rafts.iter() {
        raft.broadcast_join(0..=NUM_PEERS).await.unwrap();
    }
    noob.broadcast_join(0..=NUM_PEERS).await.unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn join_errors() {
    let mut router = Router::new(Config::testing(50), None);
    let rafts = router.add_nodes(0..5).await;

    router.create_partitions([[0], [1]]).await;
    router.initialize_nodes().await;
    await_partition_stability(&rafts[2..]).await;

    let r = rafts[0].broadcast_join(0..5).await;
    assert!(r.is_err());

    router.create_partitions([vec![0, 1, 2, 3, 4]]).await;
    await_partition_stability(&rafts[..]).await;
    await_any_leader(&rafts[..]).await;

    router.create_partitions([vec![0, 1, 2], vec![3, 4]]).await;
    await_partition_stability(&rafts[0..=2]).await;
    let leader = await_any_leader(&rafts[0..=2]).await;
    let nonleader = (leader + 1) % 2;
    rafts[nonleader as usize]
        .broadcast_join(0..5)
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn shrink_and_grow_and_shrink() {
    // tracing_subscriber::fmt::fmt()
    //     .with_max_level(tracing::Level::ERROR)
    //     .init();

    const NUM_PEERS: u64 = 5;

    let mut router = Router::new(Config::testing(100), None);
    let rafts = router.add_nodes(0..NUM_PEERS).await;
    router.initialize_nodes().await;

    // spawn_info_loop(rafts.clone(), 1000);

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
        let log = rafts[0].read_log_data_without_indexes(0).await.unwrap();
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
