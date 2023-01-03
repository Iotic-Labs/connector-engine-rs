mod config;
mod constants;

pub mod connector;
pub mod messages;
pub mod model;
pub mod model_actor;
pub mod twin;
pub mod twin_actor;

pub mod client {
    pub use iotics_grpc_client::properties;
    pub use iotics_grpc_client::twin::UpsertFeedWithMeta;
    pub use iotics_grpc_client::{
        FeedValue, GeoLocation, LangLiteral, Literal, Property, StringLiteral, Uri, Value,
    };
}
