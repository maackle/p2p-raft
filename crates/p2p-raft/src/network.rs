use std::future::Future;

use crate::{message::*, TypeCfg};

pub trait P2pNetwork<C: TypeCfg>: Clone + Send + Sync + 'static {
    fn send(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest<C>,
    ) -> impl Future<Output = P2pResponse<C>> + Send;
}
