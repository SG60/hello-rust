use config::Config;
use ferris_says::say;
use serde::{Deserialize, Serialize};
use std::io::{stdout, BufWriter};

fn ferris_say() {
    let stdout = stdout();
    let message = String::from("Hello fellow Rustaceans!");
    let width = message.chars().count();

    let mut writer = BufWriter::new(stdout.lock());
    say(message.as_bytes(), width, &mut writer).unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
struct Settings {
    notion_api_key: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ferris_say();

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

    Ok(())
}
