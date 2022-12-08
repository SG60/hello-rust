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

pub async fn get_users(client: &Client) -> Result<Vec<User>, DynamoClientError> {
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

    let users: Vec<User> = from_items(items)?;

    Ok(users)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct User {
    #[serde(rename = "userId")]
    user_id: String,
    #[serde(rename = "type")]
    record_type: String,
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
