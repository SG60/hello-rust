use serde::{Deserialize, Serialize};

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
