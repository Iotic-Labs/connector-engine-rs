use iotics_grpc_client::properties::PropertyBuilder;

use iotics_grpc_client::common::{FeedValue, Property, Visibility};
use iotics_grpc_client::properties::common_keys;
use iotics_grpc_client::twin::UpsertFeedWithMeta;

use crate::constants::{LANGUAGE, MAX_LABEL_LENGTH};

#[derive(Debug, Clone)]
pub struct Model {
    seed_prefix: String,
    label_prefix: String,
    visibility: Visibility,
    model_properties: Vec<Property>,
    feeds: Vec<UpsertFeedWithMeta>,
    twin_properties: Vec<Property>,
}

impl Model {
    pub fn new(
        seed_prefix: String,
        label_prefix: String,
        visibility: Visibility,
        model_properties: Vec<Property>,
        feeds: Vec<UpsertFeedWithMeta>,
        twin_properties: Vec<Property>,
    ) -> Self {
        Self {
            seed_prefix,
            label_prefix,
            visibility,
            model_properties,
            feeds,
            twin_properties,
        }
    }

    pub fn get_seed(&self) -> String {
        format!("{} Model", self.seed_prefix)
    }

    pub fn get_label(&self) -> String {
        format!("{} Model", self.label_prefix)
    }

    pub fn get_visibility(&self) -> Visibility {
        self.visibility
    }

    pub fn get_twin_seed(&self, twin_id: &str) -> String {
        format!("{} {}", self.seed_prefix, twin_id)
    }

    pub fn get_twin_label(&self, label: &str) -> String {
        let mut label = format!("{} {}", self.label_prefix, label)
            .trim()
            .to_string();

        while label.chars().count() > MAX_LABEL_LENGTH {
            let mut words = label.split(' ').collect::<Vec<&str>>();
            words.pop();
            label = words.join(" ");
        }

        label
    }

    pub fn get_feeds(&self, add_heartbeat: bool) -> Vec<UpsertFeedWithMeta> {
        match add_heartbeat {
            true => {
                let mut feeds = self.feeds.clone();

                feeds.push(UpsertFeedWithMeta {
                    id: "heartbeat".to_string(),
                    store_last: true,
                    values: vec![
                        FeedValue {
                            label: "timestamp".to_string(),
                            comment: "Time of the last share".to_string(),
                            data_type: "dateTime".to_string(),
                            ..Default::default()
                        },
                        FeedValue {
                            label: "shares".to_string(),
                            comment: "Number of twins for which data was shared".to_string(),
                            data_type: "integer".to_string(),
                            unit: "http://qudt.org/vocab/unit/NUM".to_string(),
                        },
                    ],
                    properties: vec![PropertyBuilder::build_label(LANGUAGE, "Heartbeat")],
                });

                feeds
            }
            false => self.feeds.clone(),
        }
    }

    pub fn get_model_properties(&self) -> &Vec<Property> {
        &self.model_properties
    }

    /// Builds the twin properties by setting values for the following fields
    /// label, model, created_at, updated_at
    /// IF they were passed `twin_properties` when the `Model` instance was created
    pub fn build_twin_properties(&self, model_did: &str, twin_label: &str) -> Vec<Property> {
        self.twin_properties
            .clone()
            .into_iter()
            .map(|property| {
                let property = match property.key.as_str() {
                    common_keys::predicate::LABEL => {
                        PropertyBuilder::build_label(LANGUAGE, twin_label)
                    }
                    common_keys::predicate::MODEL_PROPERTY => PropertyBuilder::build_uri_value(
                        common_keys::predicate::MODEL_PROPERTY,
                        model_did,
                    ),
                    _ => property,
                };

                property
            })
            .collect()
    }
}
