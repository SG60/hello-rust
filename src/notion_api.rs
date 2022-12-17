use reqwest::{header::InvalidHeaderValue, ClientBuilder};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct NotionPagesResponse {
    pub has_more: bool,
    pub next_cursor: Option<String>,
    pub object: String,
    pub results: Vec<NotionPageObject>,
    #[serde(rename = "type")]
    pub data_type: String,
    pub page: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct NotionPageObject {
    object: String,
    id: String,
    created_time: String,
    last_edited_time: String,
    created_by: serde_json::Value,
    last_edited_by: serde_json::Value,
    icon: serde_json::Value,
    parent: serde_json::Value,
    archived: bool,
    properties: serde_json::Value,
    url: String,
}

pub trait NotionReqwest {
    fn add_notion_headers(
        self,
        authorisation_token: &str,
    ) -> Result<ClientBuilder, InvalidHeaderValue>;
}
impl NotionReqwest for reqwest::ClientBuilder {
    fn add_notion_headers(
        self,
        authorisation_token: &str,
    ) -> Result<ClientBuilder, InvalidHeaderValue> {
        // Default headers for notion client
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&("Bearer ".to_string() + authorisation_token))?,
        );
        headers.insert(
            "Notion-Version",
            reqwest::header::HeaderValue::from_static("2022-06-28"),
        );

        Ok(self.default_headers(headers))
    }
}
pub fn make_notion_client(authorisation_token: &str) -> reqwest::Client {
    // client for notion requests
    reqwest::Client::builder()
        .add_notion_headers(authorisation_token)
        .expect("authorisation token should be a valid string")
        .build()
        .expect("this should work")
}
pub async fn get_pages_from_notion_database(
    authorisation_token: &str,
    database_id: &str,
) -> Result<NotionPagesResponse, reqwest::Error> {
    // client for notion requests
    let notion_client = make_notion_client(authorisation_token);

    notion_client
        .post("https://api.notion.com/v1/databases/".to_owned() + database_id + "/query")
        .send()
        .await?
        .json()
        .await
}
