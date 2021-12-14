use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use iotics_grpc_client::common::{FeedValue, LangLiteral, Property, Visibility};
use iotics_grpc_client::twin::upsert::UpsertFeedWithMeta;

use crate::connector::{literal_value_factory, uri_value_factory};
use crate::constants::predicate::{CREATED_AT_PROPERTY, MODEL_PROPERTY, UPDATED_AT_PROPERTY};
use crate::constants::MAX_LABEL_LENGTH;

#[derive(Debug, Clone)]
pub struct Model {
    pub seed_prefix: String,
    pub label_prefix: String,
    pub visibility: Visibility,
    pub model_properties: Vec<Property>,
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
                    labels: vec![LangLiteral {
                        lang: "en".to_string(),
                        value: "Heartbeat".to_string(),
                    }],
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
                    ..Default::default()
                });

                feeds
            }
            false => self.feeds.clone(),
        }
    }

    pub fn get_twin_properties(&self, model_did: &str) -> Vec<Property> {
        let time_now = OffsetDateTime::now_utc()
            .format(&Rfc3339)
            .expect("this should not happen");

        self.twin_properties
            .clone()
            .into_iter()
            .map(|property| {
                let property = match property.key.as_str() {
                    MODEL_PROPERTY => Property {
                        key: MODEL_PROPERTY.to_string(),
                        value: Some(uri_value_factory(model_did.to_string())),
                    },
                    CREATED_AT_PROPERTY => Property {
                        key: CREATED_AT_PROPERTY.to_string(),
                        value: Some(literal_value_factory(
                            "dateTime".to_owned(),
                            time_now.clone(),
                        )),
                    },
                    UPDATED_AT_PROPERTY => Property {
                        key: UPDATED_AT_PROPERTY.to_string(),
                        value: Some(literal_value_factory(
                            "dateTime".to_owned(),
                            time_now.clone(),
                        )),
                    },
                    _ => property,
                };

                property
            })
            .collect()
    }
}
