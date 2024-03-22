use std::{collections::HashMap, time::Duration};

use anyhow::Result;
use aws_sdk_dynamodb::{model::AttributeValue, types::SdkError, Client};
use serde::{Deserialize, Serialize};
use serde_dynamo::{from_item, from_items};
use thiserror::Error;
use tokio::task::JoinSet;
use tokio_stream::StreamExt;
use tracing::{trace, Instrument};
use typeshare::typeshare;

use crate::{do_with_retries, RetryConfig};

#[tracing::instrument(ret)]
pub async fn load_client() -> Client {
    let config = aws_config::load_from_env().await;
    aws_sdk_dynamodb::Client::new(&config)
}

/// Get all users from the DynamoDB table
///
/// # Errors
///
/// This function will return an error if the dynamo response fails.
#[tracing::instrument(ret, err)]
pub async fn get_users(client: &Client) -> Result<Vec<UserRecord>, DatabaseRequestError> {
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

#[tracing::instrument(err)]
pub async fn get_single_user(
    client: &Client,
    user_id: String,
) -> Result<UserRecord, DatabaseRequestError> {
    let item = client
        .get_item()
        .table_name("tasks")
        .set_key(Some(HashMap::from([
            ("userId".to_owned(), AttributeValue::S(user_id)),
            ("SK".to_owned(), AttributeValue::S("userDetails".to_owned())),
        ])))
        .send()
        .await?;

    let item = item.item().unwrap();

    let user = from_item(item.to_owned())?;

    Ok(user)
}

#[typeshare]
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

#[tracing::instrument(err)]
pub async fn get_sync_record(
    client: &Client,
    user_id: &str,
) -> Result<Vec<SyncRecord>, DatabaseRequestError> {
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

#[tracing::instrument(err)]
pub async fn get_sync_records(client: &Client) -> Result<Vec<SyncRecord>, DatabaseRequestError> {
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

#[tracing::instrument(level = "trace", ret, err, fields(n_sync_records))]
async fn get_sync_records_for_one_partition(
    client: &Client,
    partition: u16,
) -> Result<Vec<SyncRecord>, DatabaseRequestError> {
    let partition_string = "sync#".to_string() + &partition.to_string();

    let paginator = client
        .query()
        .table_name("tasks")
        .index_name("type-data-index")
        .key_condition_expression("#t = :partKey and begins_with(#s, :sortKeyValue)")
        .expression_attribute_names("#t", "type")
        .expression_attribute_names("#s", "data")
        .expression_attribute_values(":partKey", AttributeValue::S(partition_string))
        .expression_attribute_values(":sortKeyValue", AttributeValue::S("SCHEDULED".to_string()))
        .into_paginator()
        .items()
        .send();

    let items = paginator.collect::<Result<Vec<_>, _>>().await?;

    let sync_records = from_items(items)?;

    // Record the number of sync records as part of the current span.
    tracing::Span::current().record("n_sync_records", sync_records.len());

    Ok(sync_records)
}

#[tracing::instrument(ret, err, fields(n_sync_records))]
pub async fn get_sync_records_for_partitions(
    client: Client,
    partitions: Vec<u16>,
    // ) -> Result<Vec<SyncRecord>, DynamoClientError> {
) -> Result<Vec<SyncRecord>, DatabaseRequestError> {
    let mut set = JoinSet::new();

    // TODO: there should possibly be some exponential retry logic with these, incase of rate
    // limiting from DynamoDB. But it should limit the number of tries, and then just return an
    // error after that limit.

    let mut interval = tokio::time::interval(Duration::from_millis(20)); // see note below about this
    for i in partitions {
        // add a small delay before successive task spawns, to avoid overloading DynamoDB capacity
        interval.tick().await; // ticks immediately on the first time

        let client = client.clone();
        set.spawn(
            async move {
                do_with_retries(
                    || get_sync_records_for_one_partition(&client, i),
                    RetryConfig {
                        maximum_backoff: Duration::from_secs(10),
                        maximum_n_tries: Some(10),
                        ..Default::default()
                    },
                )
                .await
            }
            .in_current_span(),
        );
    }

    let mut sync_records = vec![];

    while let Some(res) = set.join_next().await {
        let mut result = res.unwrap()?;
        sync_records.append(&mut result);
    }

    trace!("{:#?}", &sync_records);

    // Record the number of sync records as part of the current span.
    tracing::Span::current().record("n_sync_records", sync_records.len());

    Ok(sync_records)
}

#[typeshare]
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

/// General Error type for all requests to Dynamo DB, including serialization errors
#[derive(Debug, Error)]
pub enum DatabaseRequestError {
    #[error("Database error")]
    DatabaseError(#[from] DynamoClientError),
    #[error("DynamoDB Serde Deserialization Error")]
    SerdeError {
        #[from]
        source: serde_dynamo::Error,
    },
}

/// Error deriving from the DynamoDB client
#[derive(Debug, Error)]
pub enum DynamoClientError {
    #[error("{0:?}")]
    QueryError(#[from] SdkError<aws_sdk_dynamodb::error::QueryError>),
    #[error("{0:?}")]
    GetItemError(#[from] SdkError<aws_sdk_dynamodb::error::GetItemError>),
}

impl<T> From<SdkError<T>> for DatabaseRequestError
where
    DynamoClientError: std::convert::From<aws_sdk_dynamodb::types::SdkError<T>>,
{
    fn from(value: SdkError<T>) -> Self {
        Self::DatabaseError(value.into())
    }
}
