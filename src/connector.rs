use async_trait::async_trait;
use iotics_grpc_client::common::{GeoLocation, Literal, Property, StringLiteral, Uri, Value};
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

// Factory function for StringLiteralValue
pub fn string_literal_value_factory(value: String) -> Value {
    Value::StringLiteralValue(StringLiteral { value })
}

// Factory function for UriValue
pub fn uri_value_factory(uri: String) -> Value {
    Value::UriValue(Uri { value: uri })
}

// Factory function for LiteralValue
pub fn literal_value_factory(data_type: String, literal: String) -> Value {
    Value::LiteralValue(Literal {
        value: literal,
        data_type,
    })
}
