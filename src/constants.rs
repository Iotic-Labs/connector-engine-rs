use std::time::Duration;

pub const AGENT_KEY_NAME: &str = "00";
pub const AGENT_TWIN_NAME: &str = "#twin-0";
pub const NEW_TWINS_SHARE_TICK_CAP: f64 = 0.75;
pub const CONCURRENT_NEW_TWINS_LIMIT: usize = 4;
pub const CONCURRENT_SHARES_LIMIT: usize = 128;
pub const RESCHEDULE_DELAY: Duration = Duration::from_millis(500);
// this should match the label max length - see PATTERN_LABEL in https://github.com/Iotic-Labs/iotic-lib-metadata
pub const MAX_LABEL_LENGTH: usize = 128;

// Connectors predicate's properties
pub mod predicate {
    pub const CREATED_FROM_PROPERTY: &str = "https://data.iotics.com/app#createdFrom";
    pub const MODEL_PROPERTY: &str = "https://data.iotics.com/app#model";
    pub const CREATED_AT_PROPERTY: &str = "https://data.iotics.com/app#createdAt";
    pub const UPDATED_AT_PROPERTY: &str = "https://data.iotics.com/app#updatedAt";
    pub const CREATED_BY_PROPERTY: &str = "https://data.iotics.com/app#createdBy";
    pub const UPDATED_BY_PROPERTY: &str = "https://data.iotics.com/app#updatedBy";
    pub const HOST_ALLOW_LIST_PROPERTY: &str = "http://data.iotics.com/public#hostAllowList";
    pub const RDF_TYPE_PROPERTY: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type";
    pub const SPACE_NAME_PROPERTY: &str = "https://data.iotics.com/app#spaceName";
    pub const COLOR_PROPERTY: &str = "https://data.iotics.com/app#color";
}

// Connectors object's properties
pub mod object {
    pub const MODEL_PROPERTY: &str = "https://data.iotics.com/app#Model";
    pub const ALL_HOST_PROPERTY: &str = "http://data.iotics.com/public#allHosts";
    pub const BY_MODEL_PROPERTY: &str = "https://data.iotics.com/app#ByModel";
    pub const BY_PUBLIC_CONNECTOR_PROPERTY: &str = "https://data.iotics.com/app#ByPublicConnector";
    pub const SET_LATER_PROPERTY: &str = "<SET-LATER>";
}
