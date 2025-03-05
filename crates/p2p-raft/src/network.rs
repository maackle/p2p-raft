use std::future::Future;

use openraft::RaftNetworkFactory;

use crate::{message::*, TypeCfg};

pub trait P2pNetwork<C: TypeCfg>: RaftNetworkFactory<C> + Clone + Send + Sync + 'static {
    fn send(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest<C>,
    ) -> impl Future<Output = P2pResponse<C>> + Send;
}
