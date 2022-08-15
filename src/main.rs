use config::Config;
use serde::{Deserialize, Serialize};
use yup_oauth2::{InstalledFlowAuthenticator, InstalledFlowReturnMethod};

#[derive(Serialize, Deserialize, Debug)]
struct Settings {
    notion_api_key: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Env vars! -----------------------------------
    let settings = Config::builder()
        .add_source(config::Environment::with_prefix("APP"))
        .build()
        .unwrap();
    let settings_map: Settings = settings.try_deserialize().unwrap();
    println!("{:#?}", settings_map);

    // Make a request to the Notion API
    let client = reqwest::Client::new();
    let res = client
        .get("https://api.notion.com/v1/users")
        .header("Notion-Version", "2022-06-28")
        .bearer_auth(settings_map.notion_api_key)
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    println!("{:#?}", res);

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
    let auth =
        InstalledFlowAuthenticator::builder(secret, InstalledFlowReturnMethod::Interactive)
            .persist_tokens_to_disk("oauthtokencache.json")
            .build()
            .await
            .unwrap();

    let scopes = &["https://www.googleapis.com/auth/calendar.readonly"];
    // token(<scopes>) is the one important function of this crate; it does everything to
    // obtain a token that can be sent e.g. as Bearer token.
    match auth.token(scopes).await {
        Ok(bearer_token) => {
            println!("The token is {:?}", bearer_token);

            // Do a request using the google token
            let res2 = client
                .get("https://www.googleapis.com/calendar/v3/calendars/primary/events?maxResults=4")
                .bearer_auth(bearer_token.as_str())
                .send()
                .await?
                .json::<serde_json::Value>()
                .await?;
            println!("{:#?}", res2);
        }
        Err(e) => println!("error: {:?}", e),
    }

    Ok(())
}
