pub use iotics_identity::{create_agent_auth_token, Config, IdentityLibError};

use crate::constants::AGENT_KEY_NAME;

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub host_address: String,
    pub identity_config: Config,
}

pub fn get_api_config() -> ApiConfig {
    dotenv::dotenv().ok();

    let parse_env = |key: &str| -> String {
        std::env::var(key).unwrap_or_else(|_| panic!("env var {} is missing", key))
    };

    ApiConfig {
        host_address: parse_env("IOTICS_HOST_ADDRESS"),
        identity_config: Config {
            resolver_address: parse_env("IOTICS_RESOLVER_ADDRESS"),
            user_did: parse_env("IOTICS_USER_DID"),
            agent_did: parse_env("IOTICS_AGENT_DID"),
            agent_key_name: AGENT_KEY_NAME.to_string(),
            agent_name: format!("#{}", parse_env("IOTICS_AGENT_NAME")),
            agent_secret: parse_env("IOTICS_AGENT_SECRET"),
            token_duration: parse_env("IOTICS_TOKEN_DURATION")
                .parse::<i64>()
                .expect("Failed to parse duration"),
        },
    }
}

pub fn get_token(api_config: &ApiConfig) -> Result<String, IdentityLibError> {
    let token = create_agent_auth_token(&api_config.identity_config)?;
    Ok(format!("bearer {}", token))
}
