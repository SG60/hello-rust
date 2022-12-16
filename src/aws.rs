use std::collections::HashMap;

use anyhow::Result;
use aws_sdk_dynamodb::{error::QueryError, model::AttributeValue, types::SdkError, Client};
use serde::{Deserialize, Serialize};
use serde_dynamo::from_items;
use thiserror::Error;
use tokio_stream::StreamExt;

pub async fn load_client() -> Client {
    let config = aws_config::load_from_env().await;
    aws_sdk_dynamodb::Client::new(&config)
}

/// Get all users from the DynamoDB table
///
/// # Errors
///
/// This function will return an error if the dynamo response fails.
pub async fn get_users(client: &Client) -> Result<Vec<UserRecord>, DynamoClientError> {
    let paginator = client
        .query()
        .table_name("tasks")
        .index_name("type-data-index")
        .key_condition_expression("#t = :partKey")
        .expression_attribute_names("#t", "type")
        .expression_attribute_values(":partKey", AttributeValue::S("userDetails".to_string()))
        .into_paginator()
        .items()
        .send();

    let items = paginator.collect::<Result<Vec<_>, _>>().await?;

    let users = from_items(items)?;

    Ok(users)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserRecord {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "type")]
    record_type: String,
    pub data: String,
    #[serde(rename = "googleRefreshToken")]
    pub google_refresh_token: Option<String>,
    #[serde(flatten)]
    pub notion_data: Option<UserRecordNotionData>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserRecordNotionData {
    // `notionB#${string}`
    #[serde(rename = "notionBotId")]
    pub notion_bot_id: String,
    #[serde(rename = "notionAccessToken")]
    pub notion_access_token: String,
}

pub async fn get_sync_record(
    client: &Client,
    user_id: &str,
) -> Result<Vec<SyncRecord>, DynamoClientError> {
    let paginator = client
        .query()
        .table_name("tasks")
        .key_condition_expression("userId = :partKey and begins_with(SK, :sk)")
        .expression_attribute_values(":partKey", AttributeValue::S(user_id.to_string()))
        .expression_attribute_values(":sk", AttributeValue::S("sync#".to_string()))
        .into_paginator()
        .items()
        .send();

    let items = paginator.collect::<Result<Vec<_>, _>>().await?;

    let sync_records = from_items(items)?;

    Ok(sync_records)
}

pub async fn get_sync_records(client: &Client) -> Result<Vec<SyncRecord>, DynamoClientError> {
    let paginator = client
        .query()
        .table_name("tasks")
        .index_name("type-data-index")
        .key_condition_expression("#t = :partKey")
        .expression_attribute_names("#t", "type")
        .expression_attribute_values(":partKey", AttributeValue::S("sync".to_string()))
        .into_paginator()
        .items()
        .send();

    let items = paginator.collect::<Result<Vec<_>, _>>().await?;

    let sync_records = from_items(items)?;

    Ok(sync_records)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncRecord {
    #[serde(rename = "userId")]
    pub user_id: String,
    #[serde(rename = "type")]
    record_type: String,
    /// includes next sync timestamp
    pub data: String,
    #[serde(rename = "lastSync")]
    pub last_sync: Option<String>,
    #[serde(rename = "notionDBProps")]
    pub notion_db_props: NotionDBPropertyOptions,
    #[serde(rename = "googleCalendar")]
    pub google_calendar: String,
    #[serde(rename = "notionDatabase")]
    pub notion_database: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NotionDBPropertyOptions {
    #[serde(rename = "notionTitleId")]
    pub notion_title_id: String,
    #[serde(rename = "notionDoneId")]
    pub notion_done_id: String,
}

#[derive(Debug, Error)]
pub enum DynamoClientError {
    #[error("DynamoDB Query Error")]
    DynamoQueryError {
        #[from]
        source: SdkError<QueryError>,
    },
    #[error("DynamoDB Serde Deserialization Error")]
    SerdeError {
        #[from]
        source: serde_dynamo::Error,
    },
}
