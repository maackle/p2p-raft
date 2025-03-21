use crate::{message::*, TypeCfg};
use openraft::RaftNetworkFactory;

#[openraft::add_async_trait]
pub trait P2pNetwork<C: TypeCfg>: RaftNetworkFactory<C> + Clone + Send + Sync + 'static {
    async fn send_rpc(
        &self,
        target: C::NodeId,
        req: P2pRequest<C>,
    ) -> anyhow::Result<P2pResponse<C>>;

    fn local_node_id(&self) -> C::NodeId;
}
