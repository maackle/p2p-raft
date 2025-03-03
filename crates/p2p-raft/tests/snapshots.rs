use p2p_raft::testing::*;

#[tokio::test(flavor = "multi_thread")]
async fn test_snapshot() {
    let (mut _router, rafts) = initialized_router(5).await;
    spawn_info_loop(rafts.clone());

    let leader = await_any_leader(&rafts).await as usize;

    rafts[leader].write_linearizable(0).await.unwrap();
    rafts[leader].write_linearizable(1).await.unwrap();
    rafts[leader].write_linearizable(2).await.unwrap();
    println!("wrote data.");

    rafts[leader].replace_snapshot(vec![5, 4, 3, 2, 1]).await;
    println!("replaced snapshot.");

    sleep(3_000).await;

    let log = rafts[leader].read_log_data().await.unwrap();
    println!("log: {log:?}");
    assert_eq!(log, vec![5, 4, 3, 2, 1]);
}
