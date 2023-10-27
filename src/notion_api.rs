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
    fn add_notion_headers(self) -> Result<ClientBuilder, InvalidHeaderValue>;
}
impl NotionReqwest for reqwest::ClientBuilder {
    fn add_notion_headers(self) -> Result<ClientBuilder, InvalidHeaderValue> {
        // Default headers for notion client
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "Notion-Version",
            reqwest::header::HeaderValue::from_static("2022-06-28"),
        );

        Ok(self.default_headers(headers))
    }
}

pub struct NotionClientUnauthenticated(reqwest::Client);
impl NotionClientUnauthenticated {
    pub fn new() -> Self {
        Self(make_notion_client())
    }

    pub async fn get_pages_from_notion_database(
        &self,
        authorisation_token: &str,
        database_id: &str,
    ) -> Result<NotionPagesResponse, reqwest::Error> {
        self.0
            .post("https://api.notion.com/v1/databases/".to_owned() + database_id + "/query")
            .add_notion_authorisation_token(authorisation_token)
            .send()
            .await?
            .json()
            .await
    }
}

trait NotionRequestBuilder {
    fn add_notion_authorisation_token(self, authorisation_token: &str) -> reqwest::RequestBuilder;
}
impl NotionRequestBuilder for reqwest::RequestBuilder {
    fn add_notion_authorisation_token(self, authorisation_token: &str) -> reqwest::RequestBuilder {
        self.header(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&("Bearer ".to_string() + authorisation_token))
                .expect("string should be valid for header"),
        )
    }
}

fn make_notion_client() -> reqwest::Client {
    // client for notion requests
    reqwest::Client::builder()
        .add_notion_headers()
        .expect("this should work")
        .build()
        .expect("this should work")
}
