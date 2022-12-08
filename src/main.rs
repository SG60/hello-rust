use reqwest::header::{self, AUTHORIZATION};
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

use hello_rust::{GoogleResponse, NotionResponse};

mod aws;
mod settings;
use aws::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Env vars! -----------------------------------
    let settings_map = settings::get_settings()?;
    println!("{:#?}", settings_map);

    let dynamo_db_client = load_client().await;

    Ok(())
}

async fn google_get_bearer_token() -> Result<yup_oauth2::AccessToken, yup_oauth2::Error> {
    // OAuth 2 Stuff ---------------------------------------------------------------------
    // Read application secret from a file. Sometimes it's easier to compile it directly into
    // the binary. The clientsecret file contains JSON like `{"installed":{"client_id": ... }}`
    let secret = yup_oauth2::read_application_secret("client_secret.json")
        .await
        .expect("client_id");

    // Create an authenticator that uses an InstalledFlow to authenticate. The
    // authentication tokens are persisted to a file named tokencache.json. The
    // authenticator takes care of caching tokens to disk and refreshing tokens once
    // they've expired.
    let auth = InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::Interactive)
        .persist_tokens_to_disk("oauthtokencache.json")
        .build()
        .await
        .unwrap();

    let scopes = &["https://www.googleapis.com/auth/calendar.readonly"];
    // token(<scopes>) is the one important function of this crate; it does everything to
    // obtain a token that can be sent e.g. as Bearer token.
    auth.token(scopes).await
}
