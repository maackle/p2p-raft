use std::collections::{BTreeSet, btree_set};

use network_impl::{router::Router, store::Request};
use p2p_raft::Dinghy;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    const N: u64 = 10;
    let all_ids = (0..N).collect::<BTreeSet<_>>();
    let mut router = Router::default();
    let (rafts, mut js) = router.add_nodes(0..N).await;
    let rafts = rafts
        .into_iter()
        .map(|r| Dinghy::from(r))
        .collect::<Vec<_>>();

    for raft in rafts.iter() {
        raft.initialize(all_ids.clone()).await.unwrap();
    }

    let leader = await_single_leader(&rafts).await as usize;
    println!("leader: {leader}");

    rafts[leader].client_write(0.into()).await.unwrap();

    // js.abort_all();
}

async fn await_leaders(rafts: &[Dinghy]) -> BTreeSet<u64> {
    loop {
        let mut leaders = BTreeSet::new();
        for raft in rafts.iter() {
            if let Some(leader) = raft.current_leader().await {
                leaders.insert(leader);
            } else {
                tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                continue;
            }
        }
        return leaders;
    }
}

async fn await_single_leader(rafts: &[Dinghy]) -> u64 {
    loop {
        let leaders = await_leaders(rafts).await;
        if leaders.len() == 1 {
            return leaders.into_iter().next().unwrap();
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}
