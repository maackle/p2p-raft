use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::Duration;

use maplit::btreemap;
use maplit::btreeset;
use openraft::ChangeMembers;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

use crate::router::RouterNode;
use crate::typ::*;
use crate::NodeId;
use crate::StateMachineStore;

pub type ResponseTx = oneshot::Sender<RpcResponse>;
pub type RequestTx = mpsc::UnboundedSender<(RpcRequest, ResponseTx)>;

#[derive(Debug, derive_more::From)]
pub enum RpcRequest {
    // App(AppRequest),
    Raft(RaftRequest),
    Management(ManagementRequest),
}

#[derive(Debug, derive_more::From, derive_more::Unwrap)]
pub enum RpcResponse {
    // App(AppResponse),
    Raft(RaftResponse),
    Management(ManagementResponse),
}

#[derive(Debug, derive_more::From)]
pub enum RaftRequest {
    Append(AppendEntriesRequest),
    Snapshot { vote: Vote, snapshot: Snapshot },
    Vote(VoteRequest),
}

#[derive(Debug, derive_more::From, derive_more::Unwrap)]
pub enum RaftResponse {
    Append(AppendEntriesResponse),
    Snapshot(SnapshotResponse),
    Vote(VoteResponse),
}

#[derive(Debug)]
pub enum ManagementRequest {
    AddLearner(NodeId),
    AddVoter(NodeId),
    RemoveVoter(NodeId),
    Init(BTreeSet<NodeId>),
    Metrics(),
}

#[derive(Debug, derive_more::From)]
pub enum ManagementResponse {
    Write(ClientWriteResponse),
    Init(()),
    Metrics(bool),
}

/// Representation of an application state.
pub struct App {
    pub id: NodeId,
    pub raft: Raft,

    /// Receive application requests, Raft protocol request or management requests.
    pub rx: mpsc::UnboundedReceiver<(RpcRequest, ResponseTx)>,
    pub node: RouterNode,

    pub state_machine: Arc<StateMachineStore>,
}

impl App {
    pub fn new(raft: Raft, node: RouterNode, state_machine: Arc<StateMachineStore>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();

        {
            let mut r = node.router.lock().unwrap();
            r.targets.insert(node.source, tx);
        }

        Self {
            id: node.source,
            raft,
            rx,
            node,
            state_machine,
        }
    }

    pub async fn run(mut self) -> Option<()> {
        loop {
            let (req, response_tx) = self.rx.recv().await?;

            let res: RpcResponse = match req {
                // Application API
                RpcRequest::Raft(RaftRequest::Append(req)) => {
                    RaftResponse::from(self.raft.append_entries(req).await.unwrap()).into()
                }
                RpcRequest::Raft(RaftRequest::Snapshot { vote, snapshot }) => RaftResponse::from(
                    self.raft
                        .install_full_snapshot(vote, snapshot)
                        .await
                        .unwrap(),
                )
                .into(),
                RpcRequest::Raft(RaftRequest::Vote(req)) => {
                    RaftResponse::from(self.raft.vote(req).await.unwrap()).into()
                }

                // Management API
                RpcRequest::Management(ManagementRequest::AddLearner(node_id)) => {
                    ManagementResponse::from(
                        self.raft.add_learner(node_id, (), true).await.unwrap(),
                    )
                    .into()
                }
                RpcRequest::Management(ManagementRequest::AddVoter(node_id)) => {
                    ManagementResponse::Write(
                        self.raft
                            .change_membership(
                                ChangeMembers::AddVoters(btreemap![node_id => ()]),
                                false,
                            )
                            .await
                            .unwrap(),
                    )
                    .into()
                }

                RpcRequest::Management(ManagementRequest::RemoveVoter(node_id)) => {
                    ManagementResponse::Write(
                        self.raft
                            .change_membership(
                                ChangeMembers::RemoveVoters(btreeset![node_id]),
                                false,
                            )
                            .await
                            .unwrap(),
                    )
                    .into()
                }

                RpcRequest::Management(ManagementRequest::Init(mut nodes)) => {
                    nodes.insert(self.id);
                    self.raft.initialize(nodes).await.unwrap();
                    ManagementResponse::Init(()).into()
                }
                // RpcRequest::Management(ManagementRequest::Metrics(req)) => {
                //     api::metrics(&mut self, req).await
                // }
                _ => panic!("unknown path: {:?}", req),
            };

            response_tx.send(res).unwrap();
        }
    }
}
