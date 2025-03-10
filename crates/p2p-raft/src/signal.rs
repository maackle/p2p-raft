use std::collections::BTreeSet;

use openraft::LogId;
use serde::{Deserialize, Serialize};

use crate::TypeCfg;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum RaftEvent<C: TypeCfg> {
    EntryCommitted(LogData<C>),
    MembershipChanged(BTreeSet<C::NodeId>),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(bound(
    serialize = "<C as openraft::RaftTypeConfig>::D: serde::Serialize",
    deserialize = "<C as openraft::RaftTypeConfig>::D: serde::de::DeserializeOwned"
))]
pub struct LogData<C: TypeCfg> {
    pub id: LogId<C>,
    pub data: C::D,
}

pub type SignalSender<C> = tokio::sync::mpsc::Sender<RaftEvent<C>>;
