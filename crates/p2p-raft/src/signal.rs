use std::collections::BTreeSet;

use openraft::{LogId, RaftTypeConfig};
use serde::{Deserialize, Serialize};

use crate::TypeCfg;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::D: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::D: serde::de::DeserializeOwned"
))]
pub enum RaftEvent<C: TypeCfg> {
    EntryCommitted {
        log_id: LogId<C>,
        data: C::D,
    },
    MembershipChanged {
        log_id: LogId<C>,
        members: BTreeSet<C::NodeId>,
    },
}

// #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
// #[serde(bound(
//     serialize = "<C as openraft::RaftTypeConfig>::D: serde::Serialize",
//     deserialize = "<C as openraft::RaftTypeConfig>::D: serde::de::DeserializeOwned"
// ))]
// pub struct LogData<C: TypeCfg> {
//     pub id: LogId<C>,
//     pub data: C::D,
// }

pub type SignalSender<C> = tokio::sync::mpsc::Sender<(<C as RaftTypeConfig>::NodeId, RaftEvent<C>)>;
pub type SignalReceiver<C> =
    tokio::sync::mpsc::Receiver<(<C as RaftTypeConfig>::NodeId, RaftEvent<C>)>;
