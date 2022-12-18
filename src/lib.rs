use serde::{Deserialize, Serialize};
use yup_oauth2::InstalledFlowAuthenticator;

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

// TODO: Make this function work!! Should take a refresh token
pub async fn google_get_bearer_token() -> Result<yup_oauth2::AccessToken, yup_oauth2::Error> {
    todo!(); // TODO: !!!

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

/// TEMPORARY!
pub fn filter_data_by_hardcoded_user_id() {
    // TEMPORARY! This is a hardcoded user_id string
    let user_id_to_use: &str = "e2TPa0rcNbgDSmPXDA8CtHlOjUN2";

    dbg!(user_id_to_use);
    todo!()
}

pub async fn get_some_data_from_google_calendar(
    bearer_auth_token: &str,
) -> Result<GoogleResponse, reqwest::Error> {
    // client for google requests
    let google_client = reqwest::Client::builder().build()?;

    // Do a request using the google token
    // TODO: make this fetch the correct calendar, rather than the primary one
    let res = google_client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events?maxResults=4")
        .bearer_auth(bearer_auth_token)
        .send()
        .await?
        .json::<GoogleResponse>()
        .await?;
    dbg!("from the google response:\n{:#?}", &res.items[0]["summary"]);

    Ok(res)
}
