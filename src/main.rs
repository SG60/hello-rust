use reqwest::header::{self, AUTHORIZATION};
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

use hello_rust::{GoogleResponse, NotionResponse};

mod settings;

use crate::settings::get_settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Env vars! -----------------------------------
    let settings_map = get_settings()?;
    println!("{:#?}", settings_map);

    // Default headers for notion client
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        reqwest::header::HeaderValue::from_str(
            &("Bearer ".to_string() + &settings_map.notion_api_key),
        )?,
    );
    headers.insert(
        "Notion-Version",
        header::HeaderValue::from_static("2022-06-28"),
    );

    // client for notion requests
    let notion_client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    // Make a request to the Notion API
    let res = notion_client
        .get("https://api.notion.com/v1/users")
        .send()
        .await?
        .json::<NotionResponse>()
        .await?;

    println!("from the Notion response:\n{:#?}", res.results[1]);

    match google_get_bearer_token().await {
        Ok(bearer_token) => {
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert(
                AUTHORIZATION,
                reqwest::header::HeaderValue::from_str(
                    &("Bearer ".to_string() + &bearer_token.as_str()),
                )?,
            );
            // client for google requests
            let google_client = reqwest::Client::builder()
                .default_headers(headers)
                .build()?;

            // Do a request using the google token
            let res2 = google_client
                .get("https://www.googleapis.com/calendar/v3/calendars/primary/events?maxResults=4")
                .bearer_auth(bearer_token.as_str())
                .send()
                .await?
                .json::<GoogleResponse>()
                .await?;
            println!("from the google response:\n{:#?}", res2.items[0]["summary"]);
        }
        Err(e) => println!("error: {:?}", e),
    }

    println!("asdadfafdasfd");

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
