use actix::Message;
use std::time::{Duration, SystemTime};

use iotics_grpc_client::Channel;

use crate::connector::ConnectorData;

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct ChannelsCreatedMessage {
    pub twin_channel: Channel,
    pub feed_channel: Channel,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct GetData {
    pub model_did: String,
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct TwinData {
    pub model_did: String,
    pub data: ConnectorData,
    pub expire_time: SystemTime,
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct HeartbeatData {
    pub model_did: String,
    pub shares: u64,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TwinCreationSuccess {
    pub twin_did: String,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TwinCreationFailure {
    pub twin_label: String,
    pub error: anyhow::Error,
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct Cleanup {
    pub delete_twins: bool,
    pub cleanup_every_secs: Duration,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TwinConcurrencyReduction {
    pub twin_seed: Option<String>,
}

#[derive(Debug, Message, Clone)]
#[rtype(result = "()")]
pub struct ShareConcurrencyReduction {
    pub shares_count: usize,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TwinDeleted;
