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
struct SlideShow {
    author: String,
    date: String,
    title: String,
    slides: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
struct MyStruct {
    slideshow: SlideShow,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ferris_say();

    let resp = reqwest::get("https://httpbin.org/json")
        .await?
        .json::<MyStruct>()
        .await?;
    println!("{:#?}", resp);
    Ok(())
}
