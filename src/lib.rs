use serde::{Deserialize, Serialize};
use yup_oauth2::InstalledFlowAuthenticator;

#[derive(Serialize, Deserialize, Debug)]
pub struct NotionResponse {
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub object: String,
    pub results: Vec<serde_json::Value>,
    #[serde(rename = "type")]
    pub t: String,
    pub user: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct GoogleResponse {
    pub items: Vec<serde_json::Value>,
    pub kind: String,
    #[serde(rename = "nextPageToken")]
    pub next_page_token: Option<String>,
    pub summary: String,
    #[serde(rename = "timeZone")]
    pub time_zone: String,
    pub updated: String,
}

pub async fn google_get_bearer_token() -> Result<yup_oauth2::AccessToken, yup_oauth2::Error> {
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
    let auth = InstalledFlowAuthenticator::builder(
        secret,
        yup_oauth2::InstalledFlowReturnMethod::Interactive,
    )
    .persist_tokens_to_disk("oauthtokencache.json")
    .build()
    .await
    .unwrap();

    let scopes = &["https://www.googleapis.com/auth/calendar.readonly"];
    // token(<scopes>) is the one important function of this crate; it does everything to
    // obtain a token that can be sent e.g. as Bearer token.
    auth.token(scopes).await
}
