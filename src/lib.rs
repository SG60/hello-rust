use std::future::Future;

use serde::{Deserialize, Serialize};
use tracing::{event, span, Level};

use cluster_management::etcd::{create_lease, lease_keep_alive, EtcdClients};

pub mod aws;
pub mod cluster_management;
pub mod notion_api;
pub mod settings;
pub mod tracing_utils;

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

#[derive(Debug)]
pub struct GoogleToken {
    pub refresh_token: String,
    pub access_token: Option<GoogleAccessToken>,
}

#[derive(Debug)]
pub struct GoogleAccessToken {
    pub access_token: String,
    pub expiry_time: std::time::SystemTime,
}

#[derive(Serialize, Deserialize, Debug)]
struct GoogleRefreshTokenRequestResponse {
    access_token: String, // e.g. "1/fFAasGRNJTz70BzhT3Zg"
    /// in seconds
    expires_in: u64, // e.g. 3920
    scope: String,        // e.g. "https://www.googleapis.com/auth/drive.metadata.readonly"
    token_type: String,   // always "Bearer"
}

impl GoogleToken {
    pub fn new(refresh_token: &str) -> Self {
        Self {
            refresh_token: refresh_token.to_owned(),
            access_token: None,
        }
    }

    /// Refresh the access token
    ///
    /// # Errors
    ///
    /// This function can return an error for several reasons: the request to google fails, the
    /// refresh token is invalid, or the response from google does not match the serde struct.
    ///
    /// TODO: return something different for some of these errors
    pub async fn refresh_token(
        &mut self,
        google_oauth_client_id: &str,
        google_oauth_client_secret: &str,
    ) -> Result<&Self, reqwest::Error> {
        // POST /token HTTP/1.1
        // Host: oauth2.googleapis.com
        // Content-Type: application/x-www-form-urlencoded
        //
        // client_id=your_client_id&
        // client_secret=your_client_secret&
        // refresh_token=refresh_token&
        // grant_type=refresh_token
        let client = reqwest::Client::builder().build()?;
        let params = [
            ("client_id", google_oauth_client_id),
            ("client_secret", google_oauth_client_secret),
            ("refresh_token", &self.refresh_token),
            ("grant_type", "refresh_token"),
        ];
        let response_json = client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
            .map(|response| response.json::<GoogleRefreshTokenRequestResponse>());

        let response_json = response_json?.await?;

        let expires_in = std::time::Duration::from_secs(response_json.expires_in); // TODO: expiry time
        let expiry_time = std::time::SystemTime::now() + expires_in;

        self.access_token = Some(GoogleAccessToken {
            access_token: response_json.access_token,
            expiry_time,
        });

        Ok(self)
    }

    pub async fn get(
        &mut self,
        google_oauth_client_id: &str,
        google_oauth_client_secret: &str,
    ) -> String {
        let mut expired = false;
        if let Some(ref access_token) = self.access_token {
            if access_token.expiry_time <= std::time::SystemTime::now() {
                expired = true
            }
        } else {
            expired = true
        };

        let _refresh_response = if expired {
            println!("Refreshing Google Calendar user access token");
            Some(
                self.refresh_token(google_oauth_client_id, google_oauth_client_secret)
                    .await,
            )
        } else {
            None
        };

        self.access_token
            .as_ref()
            .expect("Access token should exist")
            .access_token
            .to_owned()
    }
}

/// TEMPORARY!?! Useful for testing though.
pub fn filter_data_by_hardcoded_user_id(users: &[aws::UserRecord]) -> Option<&aws::UserRecord> {
    // TEMPORARY! This is a hardcoded user_id string
    let user_id_to_use = "e2TPa0rcNbgDSmPXDA8CtHlOjUN2".to_string();

    let filtered_user = users
        .iter()
        .find(|element| element.user_id == user_id_to_use);

    filtered_user
}

pub async fn get_some_data_from_google_calendar(
    bearer_auth_token: &str,
) -> Result<GoogleResponse, reqwest::Error> {
    // client for google requests
    let google_client = reqwest::Client::builder().build()?;

    // Do a request using the google token
    // TODO: make this fetch the correct calendar, rather than the primary one
    let res = google_client
        .get("https://www.googleapis.com/calendar/v3/calendars/primary/events?maxResults=4")
        .bearer_auth(bearer_auth_token)
        .send()
        .await?
        .json::<GoogleResponse>()
        .await?;
    dbg!("from the google response:\n{:#?}", &res.items[0]["summary"]);

    Ok(res)
}

pub async fn do_with_retries<A, Fut, E, F: Fn() -> Fut>(f: F) -> Result<A, E>
where
    E: std::error::Error,
    Fut: Future<Output = Result<A, E>>,
{
    let mut retry_wait_seconds = 1;

    loop {
        let result = f().await;

        match result {
            Err(error) => {
                event!(
                    Level::WARN,
                    "Error, trying again. Waiting {} seconds. {}",
                    retry_wait_seconds,
                    error
                );

                tokio::time::sleep(std::time::Duration::from_secs(retry_wait_seconds)).await;
                if retry_wait_seconds < 300 {
                    retry_wait_seconds += retry_wait_seconds
                };
            }
            Ok(result) => break Ok(result),
        }
    }
}

#[tracing::instrument]
pub async fn do_some_stuff_with_etcd(
    etcd_endpoint: &str,
) -> cluster_management::Result<EtcdClients> {
    event!(Level::INFO, "Initialising etcd grpc clients");
    let mut etcd_clients =
        do_with_retries(|| EtcdClients::connect(etcd_endpoint.to_owned())).await?;

    let lease = create_lease(etcd_clients.lease.clone()).await?;
    event!(
        Level::INFO,
        etcd_lease_id = lease.id,
        "current lease: {:#?}",
        lease.id
    );

    let span = span!(Level::INFO, "Starting lease_keep_alive task");
    span.in_scope(|| {
    let _result_of_tokio_task =
        tokio::spawn(lease_keep_alive(etcd_clients.lease.clone(), lease.id));
    dbg!(_result_of_tokio_task);
    });


    let kv_response = cluster_management::record_node_membership(&mut etcd_clients, lease.id)
        .await
        .map_err(|e| {
            event!(Level::ERROR, "{:#?}", e);
            e
        })?;

    Ok(etcd_clients)
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn fake_test() {
        let result = 2 + 2;
        assert_eq!(result, 4);
    }
}
