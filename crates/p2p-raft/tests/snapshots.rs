use p2p_raft::{testing::*, Config};

#[tokio::test(flavor = "multi_thread")]
#[ignore = "waiting on resolution of https://github.com/databendlabs/openraft/issues/1333#issuecomment-2705359074"]
async fn test_snapshot() -> anyhow::Result<()> {
    const NUM_PEERS: u64 = 5;
    let mut router = Router::new(Config::default(), None);
    let rafts = router.add_nodes(0..NUM_PEERS).await;
    router.initialize_nodes().await;

    spawn_info_loop(rafts.clone(), 1000, true);

    let l = await_any_leader(&rafts).await as usize;
    let leader = &rafts[l];

    leader.write_linearizable(0).await?;
    leader.write_linearizable(1).await?;
    leader.write_linearizable(2).await?;
    println!("wrote data.");

    leader.replace_snapshot(vec![5, 4, 3, 2, 1]).await;
    println!("replaced snapshot.");

    let snapshot = leader.get_snapshot().await?;
    assert_eq!(snapshot.unwrap().snapshot, vec![5, 4, 3, 2, 1]);
    assert_eq!(
        leader.read_log_data_without_indexes(0).await?,
        Vec::<u64>::new()
    );

    for i in 0..NUM_PEERS as usize {
        if i == l {
            continue;
        }
        let r = &rafts[i];
        let log = r.read_log_entries(0).await?;
        println!("log: {log:?}");
        todo!("ensure snapshot is propagated");
        // assert_eq!(log, vec![5, 4, 3, 2, 1]);
    }

    Ok(())
}
