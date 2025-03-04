use std::future::Future;

use crate::{message::*, TypeConf};

pub trait P2pNetwork<C: TypeConf> {
    fn send(
        &self,
        source: C::NodeId,
        target: C::NodeId,
        req: P2pRequest<C>,
    ) -> impl Future<Output = P2pResponse<C>>;
}
