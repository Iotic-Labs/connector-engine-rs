use std::sync::{Arc, Mutex};

use iotics_grpc_client::IntoAuthBuilder;
use iotics_identity::{create_agent_auth_token, Config};

use crate::constants::AGENT_KEY_NAME;

#[derive(Debug, Clone)]
pub struct AuthBuilder {
    api_config: Arc<Mutex<ApiConfig>>,
    token: Arc<Mutex<Option<String>>>,
}

impl AuthBuilder {
    pub fn new() -> Arc<Self> {
        let api_config = get_api_config();

        Arc::new(Self {
            api_config: Arc::new(Mutex::new(api_config)),
            token: Arc::new(Mutex::new(None)),
        })
    }

    pub fn get_identity_config(&self) -> Result<Config, anyhow::Error> {
        let api_config_lock = self
            .api_config
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock the settings mutex"))?;

        Ok(api_config_lock.identity_config.clone())
    }
}

impl IntoAuthBuilder for AuthBuilder {
    fn get_host(&self) -> Result<String, anyhow::Error> {
        let api_config_lock = self
            .api_config
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock the settings mutex"))?;

        Ok(api_config_lock.host_address.clone())
    }

    fn get_token(&self) -> Result<String, anyhow::Error> {
        let mut token_lock = self
            .token
            .lock()
            .map_err(|_| anyhow::anyhow!("failed to lock the token mutex"))?;

        if token_lock.is_none() {
            let api_config_lock = self
                .api_config
                .lock()
                .map_err(|_| anyhow::anyhow!("failed to lock the settings mutex"))?;

            let identity_config = Config {
                resolver_address: api_config_lock.identity_config.resolver_address.clone(),
                token_duration: api_config_lock.identity_config.token_duration,
                user_did: api_config_lock.identity_config.user_did.clone(),
                agent_did: api_config_lock.identity_config.agent_did.clone(),
                agent_key_name: api_config_lock.identity_config.agent_key_name.clone(),
                agent_name: api_config_lock.identity_config.agent_name.clone(),
                agent_secret: api_config_lock.identity_config.agent_secret.clone(),
            };

            let token = create_agent_auth_token(&identity_config)?;
            let token = format!("bearer {}", token);

            token_lock.replace(token);
        }

        let token = token_lock.as_ref().expect("this should never happen");

        Ok(token.clone())
    }
}

#[derive(Debug, Clone)]
struct ApiConfig {
    host_address: String,
    identity_config: Config,
}

fn get_api_config() -> ApiConfig {
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
