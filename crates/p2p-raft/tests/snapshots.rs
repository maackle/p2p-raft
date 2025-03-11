use p2p_raft::{testing::*, DinghyConfig};

#[tokio::test(flavor = "multi_thread")]
async fn test_snapshot() {
    const NUM_PEERS: u64 = 5;
    let mut router = Router::new(DinghyConfig::default(), None);
    let rafts = router.add_nodes(0..NUM_PEERS).await;
    router.initialize_nodes().await;

    spawn_info_loop(rafts.clone(), 1000);

    let leader = await_any_leader(&rafts).await as usize;

    rafts[leader].write_linearizable(0).await.unwrap();
    rafts[leader].write_linearizable(1).await.unwrap();
    rafts[leader].write_linearizable(2).await.unwrap();
    println!("wrote data.");

    todo!();
    // rafts[leader].replace_snapshot(vec![5, 4, 3, 2, 1]).await;
    println!("replaced snapshot.");

    sleep(3_000).await;

    let log = rafts[leader].read_log_data().await.unwrap();
    println!("log: {log:?}");
    assert_eq!(log, vec![5, 4, 3, 2, 1]);
}
