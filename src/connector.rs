use async_trait::async_trait;
use iotics_grpc_client::common::{GeoLocation, Property};
use serde_json::Value as SerdeValue;
use std::collections::HashMap;
use std::fmt::Debug;

#[async_trait]
pub trait Connector: Debug {
    async fn get_data(&self) -> Result<Vec<ConnectorData>, anyhow::Error>;
}

#[derive(Debug, Clone)]
pub struct ConnectorData {
    pub id: String,
    pub label: String,
    pub location: Option<GeoLocation>,
    pub feeds: HashMap<String, SerdeValue>,
    pub properties: Vec<Property>,
}

// Convert a String object into an f64 if "field" contains a number or return None otherwise
pub fn parse_to_float(field: String) -> Option<f64> {
    if !field.is_empty() {
        field.parse::<f64>().ok()
    } else {
        None
    }
}
