use actix::Message;
use std::time::SystemTime;

use iotics_grpc_client::common::Channel;
use iotics_grpc_client::twin::{FeedApiClient, TwinApiClient};

use crate::connector::ConnectorData;

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct ChannelsCreatedMessage {
    pub twin_channel: TwinApiClient<Channel>,
    pub feed_channel: FeedApiClient<Channel>,
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
pub struct TwinCreated {
    pub twin_did: String,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct Cleanup {
    pub delete_twins: bool,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TwinConcurrencyReduction {
    pub twin_seed: Option<String>,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct ShareConcurrencyReduction {
    pub amount: usize,
}

#[derive(Debug, Message)]
#[rtype(result = "()")]
pub struct TwinDeleted;
