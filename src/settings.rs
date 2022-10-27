use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub notion_api_key: String,
}

pub fn get_settings() -> Result<Settings, ConfigError> {
    // Env vars! -----------------------------------
    let settings = Config::builder()
        .add_source(config::Environment::with_prefix("APP"))
        .build()
        .unwrap();

    return settings.try_deserialize();
}
