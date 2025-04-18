use std::time::Duration;

use itertools::Itertools;
use openraft::ServerState;

use crate::network::P2pNetwork;

#[allow(warnings)]
pub fn spawn_info_loop<C, N>(
    rafts: impl IntoIterator<Item = crate::P2pRaft<C, N>>,
    poll_interval_ms: u64,
    verbose: bool,
) where
    C: crate::TypeCfg<Entry = openraft::Entry<C>>,
    N: P2pNetwork<C>,
{
    let rafts = rafts.into_iter().collect_vec();
    tokio::spawn({
        let mut interval = tokio::time::interval(Duration::from_millis(poll_interval_ms));
        async move {
            loop {
                interval.tick().await;
                println!();
                println!("........................................................");
                for r in rafts.iter() {
                    println!("{}", r.debug_line(verbose).await);
                }
                println!("........................................................");
                println!();
            }
        }
    });
}

pub async fn await_any_leader_t<C, N>(
    rafts: impl IntoIterator<Item = impl std::ops::Deref<Target = crate::P2pRaft<C, N>>>,
    timeout: Option<Duration>,
) -> anyhow::Result<C::NodeId>
where
    C: crate::TypeCfg<Entry = openraft::Entry<C>>,
    N: P2pNetwork<C>,
{
    let rafts = rafts.into_iter().map(|r| r.deref().clone()).collect_vec();
    let start = std::time::Instant::now();
    let ids = rafts.iter().map(|r| r.id.clone()).collect_vec();
    println!("awaiting any leader for {ids:?}");
    let futs = rafts.iter().map(|r| {
        Box::pin(async move {
            r.raft
                .wait(timeout)
                .state(ServerState::Leader, "await_state")
                .await
        })
    });

    let (res, _idx, _) = futures::future::select_all(futs).await;
    let leader = res?.id;
    println!("found new leader {leader} in {:?}", start.elapsed());
    Ok(leader)
}

pub async fn await_any_leader<C, N>(
    rafts: impl IntoIterator<Item = impl std::ops::Deref<Target = crate::P2pRaft<C, N>>>,
) -> C::NodeId
where
    C: crate::TypeCfg<Entry = openraft::Entry<C>>,
    N: P2pNetwork<C>,
{
    let rafts = rafts.into_iter().map(|r| r.deref().clone()).collect_vec();
    let start = std::time::Instant::now();
    let ids = rafts.iter().map(|r| r.id.clone()).collect_vec();
    println!("awaiting any leader for {ids:?}");
    let futs = rafts.iter().map(|r| {
        Box::pin(async move {
            r.raft
                .wait(None)
                .state(ServerState::Leader, "await leader")
                .await
        })
    });

    let (res, _idx, _) = futures::future::select_all(futs).await;
    let leader = res.unwrap().id;
    let election_time = start.elapsed();

    futures::future::join_all(rafts.iter().map(|r| {
        let leader = leader.clone();
        async move {
            if r.id != leader {
                r.wait(None)
                    .current_leader(leader, "await consensus")
                    .await
                    .unwrap();
            }
        }
    }))
    .await;
    let consensus_time = start.elapsed();

    println!(
        "elected new leader {leader} in {:?}, full consensus in {:?}",
        election_time, consensus_time
    );
    leader
}

pub async fn await_partition_stability<C, N>(
    rafts: impl IntoIterator<Item = impl std::ops::Deref<Target = crate::P2pRaft<C, N>>>,
) where
    C: crate::TypeCfg<Entry = openraft::Entry<C>>,
    N: P2pNetwork<C>,
{
    let rafts = rafts.into_iter().map(|r| r.deref().clone()).collect_vec();
    let start = std::time::Instant::now();
    let ids = rafts.iter().map(|r| r.id.clone()).collect_vec();

    println!("~~ awaiting stability for partition {ids:?}",);

    futures::future::join_all(rafts.iter().map(|r| {
        let ids = ids.clone();
        async move { r.wait(None).voter_ids(ids, "partition stability").await }
    }))
    .await;

    println!(
        "~~ partition {ids:?} stabilized in {:?} ~~~~~~~~~~~~~~~~~~~~~~",
        start.elapsed()
    );

    // somehow this little sleep is needed
    sleep(100).await;
}

pub async fn sleep(ms: u64) {
    tokio::time::sleep(Duration::from_millis(ms)).await;
}

pub fn setup_tracing(directive: &str) {
    tracing_subscriber::fmt::fmt()
        .with_file(true)
        .with_line_number(true)
        // .with_max_level(Level::DEBUG)
        .with_env_filter(tracing_subscriber::EnvFilter::try_new(directive).unwrap())
        .init();
}
