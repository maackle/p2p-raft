use std::{
    collections::{BTreeSet, HashMap, HashSet},
    sync::Arc,
};

use parking_lot::Mutex;
use partitioner::{NodeId, Router, RouterNode};
use tokio::time::Instant;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let mut router = Router::<Node>::default();
    router.add_nodes((0..10).map(|id| {
        (id, Node {
            id,
            state: Arc::new(Mutex::new(NodeState::default())),
        })
    }));

    router.partitions(vec![0..5, 5..10]);

    router.with_nodes(|node| node.start(router.clone()));

    loop {
        if router
            .lock()
            .nodes()
            .all(|(_, node)| node.state.lock().visible.len() == 10)
        {
            break;
        } else {
            router.lock().nodes().for_each(|(id, node)| {
                println!(
                    "{}: {:?}",
                    id,
                    node.state.lock().visible.keys().collect::<BTreeSet<_>>()
                );
            });
            println!("---");
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
}

#[derive(Debug, Clone)]
pub struct Node {
    id: NodeId,
    state: Arc<Mutex<NodeState>>,
}

#[derive(Debug, Default)]
pub struct NodeState {
    visible: HashMap<NodeId, Instant>,
}

impl NodeState {
    pub fn handle_heartbeat(&mut self, id: NodeId) {}
}

impl RouterNode for Node {}

impl Node {
    fn start(&self, router: Router<Self>) {
        let id = self.id;
        let state = self.state.clone();
        tokio::spawn(async move {
            loop {
                let r = router
                    .broadcast(id, move |node| {
                        node.state.lock().handle_heartbeat(id);
                    })
                    .await
                    .join_all()
                    .await;
                for (id, r) in r {
                    match r {
                        Ok(()) => {
                            state.lock().visible.insert(id, Instant::now());
                        }
                        Err(e) => {
                            println!("{}: {:?}", id, e);
                        }
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
            }
        });
    }
}
