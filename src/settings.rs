use config::{Config, ConfigError};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub google_oauth_client_id: String,
    pub google_oauth_client_secret: String,

    /// URL for the etcd instance for cluster coordination. Only used if `clustered` is `true`.
    pub etcd_url: Option<String>,
    pub clustered: bool,
}

#[tracing::instrument]
pub fn get_settings() -> Result<Settings, ConfigError> {
    // Env vars! -----------------------------------
    let settings = Config::builder()
        .set_default("clustered", "true")?
        .add_source(config::Environment::with_prefix("APP"))
        .build()
        .unwrap();

    settings.try_deserialize()
}
