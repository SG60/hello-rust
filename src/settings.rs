use figment::{
    providers::{Env, Format, Toml},
    Figment,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct Settings {
    pub google_oauth_client_id: String,
    pub google_oauth_client_secret: String,

    /// URL for the etcd instance for cluster coordination. Only used if `clustered` is `true`.
    pub etcd_url: Option<String>,
    #[serde(default = "clustered_default")]
    pub clustered: bool,

    pub node_name: String,
}

fn clustered_default() -> bool {
    true
}

#[tracing::instrument]
pub fn get_settings() -> Result<Settings, figment::Error> {
    Figment::new()
        .merge(Toml::file("hello-rust-config.toml"))
        .merge(Env::prefixed("APP_"))
        // fallbacks
        .join(Env::raw().only(&["HOSTNAME"]).map(|_| "node_name".into()))
        .extract()
}
