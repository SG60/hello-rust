use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub google_oauth_client_id: String,
    pub google_oauth_client_secret: String,
}

#[tracing::instrument]
pub fn get_settings() -> Result<Settings, ConfigError> {
    // Env vars! -----------------------------------
    let settings = Config::builder()
        .add_source(config::Environment::with_prefix("APP"))
        .build()
        .unwrap();

    settings.try_deserialize()
}
