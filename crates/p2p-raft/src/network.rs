use openraft::{error::RPCError, RaftNetworkFactory};

use crate::{message::*, TypeCfg};

#[openraft::add_async_trait]
pub trait P2pNetwork<C: TypeCfg>: RaftNetworkFactory<C> + Clone + Send + Sync + 'static {
    async fn send_p2p(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest<C>,
    ) -> Result<P2pResponse<C>, RPCError<C>>;
}
