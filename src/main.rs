use config::Config;
use serde::{Deserialize, Serialize};

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

    let res2 = client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events?maxResults=4")
        .bearer_auth("testtoken")
        .send()
        .await?
        .json::<serde_json::Value>()
        .await?;
    println!("{}", res2);

    Ok(())
}
