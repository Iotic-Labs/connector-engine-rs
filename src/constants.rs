use std::time::Duration;

pub const AGENT_KEY_NAME: &str = "00";
pub const AGENT_TWIN_NAME: &str = "#twin-0";
pub const NEW_TWINS_SHARE_TICK_CAP: f64 = 0.75;
pub const CONCURRENT_NEW_TWINS_LIMIT: usize = 4;
pub const CONCURRENT_SHARES_LIMIT: usize = 128;
pub const RESCHEDULE_DELAY: Duration = Duration::from_millis(500);
// this should match the label max length - see PATTERN_LABEL in https://github.com/Iotic-Labs/iotic-lib-metadata
pub const MAX_LABEL_LENGTH: usize = 128;
pub const LANGUAGE: &str = "en";
