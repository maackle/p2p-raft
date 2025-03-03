use crate::message::*;
use openraft::RaftTypeConfig;

pub trait P2pNetwork<C: RaftTypeConfig> {
    fn send(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest,
    ) -> impl Future<Output = anyhow::Result<()>>;
}
