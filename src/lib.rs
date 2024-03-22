use std::{collections::HashMap, future::Future, time::Duration};

use anyhow::{anyhow, Result};
use aws::get_users;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;
use tracing::{
    debug, debug_span, error, event, info_span, instrument, span, trace, Instrument, Level,
};

use crate::{
    aws::get_sync_records_for_partitions,
    cluster_management::{
        establish_correct_sync_partition_locks, initialise_lease_and_node_membership,
    },
    etcd::EtcdClients,
};

pub mod aws;
pub mod cluster_management;
pub mod etcd;
pub mod notion_api;
pub mod settings;
mod source_gcal;
mod source_notion;

pub async fn run(mut shutdown_rx: tokio::sync::watch::Receiver<()>) -> anyhow::Result<()> {
    let init_stuff_that_can_be_shutdown_immediately = async move {
        opentelemetry_tracing_utils::set_up_logging()?;

        // Env vars! -----------------------------------
        event!(Level::INFO, "Looking for settings.");
        let settings_map = do_with_retries_sync(
            settings::get_settings,
            RetryConfig {
                maximum_backoff: Duration::from_secs(300),
                ..Default::default()
            },
        )
        .await
        .unwrap();

        event!(Level::INFO, "Settings successfully obtained.");
        event!(Level::INFO, "{:#?}", settings_map);

        dbg!(std::env::var("NO_OTLP")
            .unwrap_or_else(|_| "0".to_owned())
            .as_str());

        anyhow::Ok::<_>(settings_map)
    };

    let settings_map = tokio::select! {
        result = init_stuff_that_can_be_shutdown_immediately => {
            Some(result.unwrap())
        },
        s = shutdown_rx.changed() => {
            s.expect("receiver should work");
            event!(Level::INFO, "rx shutdown channel changed");
            None
        }
    };

    if let Some(settings_map) = settings_map {
        let span = span!(Level::TRACE, "talk to etcd");

        let node_name = settings_map.node_name;

        let result_of_work = async {
            // This is correct! If we yield here, the span will be exited,
            // and re-entered when we resume.
            if settings_map.etcd_url.is_some() {
                event!(Level::INFO, "About to try talking to etcd!");

                event!(Level::INFO, "Clustered setting: {}", settings_map.clustered);

                let shutdown_receiver = shutdown_rx.clone();

                let result = do_some_stuff_with_etcd_and_init(
                    &settings_map.etcd_url.expect("should be valid string"),
                    node_name.as_str(),
                    shutdown_receiver,
                )
                .await;

                match result {
                    Ok(ref result) => {
                        event!(Level::INFO, "{:#?}", result);
                    }
                    Err(ref error) => {
                        event!(Level::ERROR, "Error while talking to etcd. {:#?}", error)
                    }
                }
                result.ok()
            } else {
                event!(Level::WARN, "No etcd endpoint set.");
                None
            }
        }
        // instrument the async block with the span...
        .instrument(span)
        // ...and await it.
        .await;

        let mut rx2 = shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = async move {
                    loop {
                        event!(Level::INFO, "a loop");
                        tokio::time::sleep(Duration::from_secs(10)).await;
                    }
                }
                    .instrument(span!(Level::TRACE, "loop span")) => {},
                _ = rx2.changed() => {
                    event!(Level::INFO, "rx shutdown channel changed");
                }
            }
        });

        let result_of_work_join_handle =
            result_of_work.expect("Should have a join handle (check that etcd endpoint is set)");

        result_of_work_join_handle.await?;
    }

    Ok(())
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

pub async fn do_with_retries_infinite<A, Fut, E, F: Fn() -> Fut>(f: F) -> A
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
            Ok(result) => break result,
        }
    }
}

#[derive(Debug)]
struct RetryConfig {
    maximum_backoff: Duration,
    maximum_n_tries: Option<u32>,
    initial_duration: Duration,
}
impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            maximum_backoff: Duration::from_secs(30),
            maximum_n_tries: None,
            initial_duration: Duration::from_millis(5),
        }
    }
}

#[instrument(err(Debug), skip(f), level = "trace")]
async fn do_with_retries<A, Fut, E, F: Fn() -> Fut>(f: F, config: RetryConfig) -> Result<A, E>
where
    E: std::error::Error,
    Fut: Future<Output = Result<A, E>>,
{
    let mut n_tries = 0;
    let mut retry_wait_duration = config.initial_duration;

    loop {
        let result = f().await;

        match result {
            Err(error) => {
                n_tries += 1;

                trace!(n_tries, "{}", error);

                if let Some(max) = config.maximum_n_tries {
                    if n_tries == max {
                        break Err(error);
                    };
                }

                tokio::time::sleep(retry_wait_duration).await;
                if retry_wait_duration < config.maximum_backoff {
                    retry_wait_duration *= 2;
                };
            }
            Ok(result) => {
                break Ok(result);
            }
        }
    }
}
#[instrument(err(Debug), skip(f), level = "trace")]
async fn do_with_retries_sync<A, E, F: Fn() -> Result<A, E>>(
    f: F,
    config: RetryConfig,
) -> Result<A, E>
where
    E: std::error::Error,
{
    let mut n_tries = 0;
    let mut retry_wait_duration = config.initial_duration;

    loop {
        let result = f();

        match result {
            Err(error) => {
                n_tries += 1;

                trace!(n_tries, "{}", error);

                if let Some(max) = config.maximum_n_tries {
                    if n_tries == max {
                        break Err(error);
                    };
                }

                tokio::time::sleep(retry_wait_duration).await;
                if retry_wait_duration < config.maximum_backoff {
                    retry_wait_duration *= 2;
                };
            }
            Ok(result) => {
                break Ok(result);
            }
        }
    }
}

#[derive(Debug)]
pub struct InitAndEtcdTaskReturn {
    pub etcd_clients: EtcdClients,
    pub result_of_tokio_task: tokio::task::JoinHandle<()>,
}

/// Spawns another thread that does cluster membership and starting the sync process
#[tracing::instrument]
pub async fn do_some_stuff_with_etcd_and_init(
    etcd_endpoint: &str,
    node_name: &str,
    mut shutdown_receiver: tokio::sync::watch::Receiver<()>,
) -> anyhow::Result<tokio::task::JoinHandle<()>> {
    event!(Level::INFO, "Initialising etcd grpc clients");
    let etcd_clients = tokio::select! {
        x = do_with_retries_infinite(|| EtcdClients::connect(etcd_endpoint.to_owned())) => {Some(x)},
        _ = shutdown_receiver.changed() => {None}
    };

    let etcd_clients = etcd_clients.ok_or(anyhow!("Shutdown, so no etcd clients available"))?;

    let result_of_tokio_task = tokio::spawn(manage_cluster_node_membership_and_start_work(
        etcd_clients,
        node_name.to_owned(),
        shutdown_receiver,
    ));

    Ok(result_of_tokio_task)
}

/// Manage cluster membership recording
///
/// Uses [initialise_lease_and_node_membership] and various lease functions.
///
/// Doesn't return a result, so that it can run nicely in a separate tokio task. Will just retry
/// the whole thing if any part fails.
async fn manage_cluster_node_membership_and_start_work(
    etcd_clients: EtcdClients,
    node_name: String,
    mut shutdown_receiver: tokio::sync::watch::Receiver<()>,
) {
    let token = CancellationToken::new();
    let cloned_token = token.clone();

    tokio::spawn(async move {
        shutdown_receiver.changed().await.unwrap();
        event!(
            Level::DEBUG,
            "shutdown received, triggering cancellation token"
        );
        cloned_token.cancel();
    });

    // initialising the dynamo db client is expensive, so should only be done once
    let dynamo_db_client = aws::load_client().await;

    loop {
        let mut lease = Default::default();
        let result = initialise_lease_and_node_membership(etcd_clients.clone(), node_name.clone())
            .await
            .map(|x| lease = x);

        match result {
            Ok(_) => {
                let lease_keep_alive_join_handle = tokio::spawn(crate::etcd::lease_keep_alive(
                    etcd_clients.clone().lease,
                    lease.id,
                ));
                let run_work_join_handle = tokio::spawn(start_sync_pipeline(
                    etcd_clients.clone(),
                    node_name.clone(),
                    lease.id,
                    dynamo_db_client.clone(),
                ));

                tokio::select! {
                    handle = lease_keep_alive_join_handle => {
                        let result = handle.unwrap();
                        dbg!("lease_keep_alive_join_handle completed!");

                        if result.is_err() {
                            println!("Error with lease_keep_alive, will create a new lease")
                        };
                    },
                    handle = run_work_join_handle => {
                        dbg!("run_work_join_handle completed!");
                        match handle.expect("join result should be valid") {
                            Ok(inner) => {
                                dbg!{inner};
                            },
                            Err(error) => {
                                error!{"Error in running work"};
                                dbg!{error};
                            },
                        };
                        break
                    },
                    _ = token.cancelled() => {
                        event!(Level::INFO, "received shutdown message, ending event loop");
                        break
                    }
                };
            }
            Err(e) => {
                event!(
                    Level::ERROR,
                    "Error initialising cluster membership, will try again. Error: {e:#?}"
                );
            }
        };

        dbg!("Reached end of event loop");

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

pub async fn start_sync_pipeline(
    mut etcd_clients: EtcdClients,
    node_name: String,
    current_lease: i64,
    dynamo_db_client: aws_sdk_dynamodb::Client,
) -> Result<std::convert::Infallible> {
    let start_span = info_span!("set up pipeline");

    let (_reqwest_client, mut user_creds) = start_span.in_scope(|| {
        // Client is cheap to clone and uses a pool, so it is better to just use one for everything!
        let reqwest_client = reqwest::Client::new();

        let user_creds: HashMap<String, aws::UserRecord> = HashMap::new();

        (reqwest_client, user_creds)
    });

    // NOTE: THIS IS JUST HERE FOR TESTING
    let users = get_users(&dynamo_db_client).await?;
    dbg!(users);

    loop {
        let pipeline_span = info_span!("sync pipeline");
        pipeline_span.follows_from(&start_span);

        let sync_job = async {
            let sync_partition_lock_records = establish_correct_sync_partition_locks(
                &mut etcd_clients.kv,
                node_name.as_str(),
                current_lease,
            )
            .await;

            let db_sync_records = get_sync_records_for_partitions(
                dynamo_db_client.clone(),
                sync_partition_lock_records,
            )
            .await?;

            // NOTE: This should run in a task
            // see:
            // https://medium.com/@polyglot_factotum/rust-concurrency-a-streaming-workflow-served-with-a-side-of-back-pressure-955bdf0266b5
            //
            // TODO: communicate between source and processor over channels
            // could use this: https://docs.rs/async-channel/latest/async_channel/
            for i in db_sync_records {
                let single_sync_job_span = info_span!("single sync job");
                async {
                    dbg!(&i);

                    let user_id = i.user_id.clone();

                    let current_user_creds = user_creds.get(&user_id);
                    let current_user_creds = match current_user_creds {
                        None => {
                            let user =
                                aws::get_single_user(&dynamo_db_client, user_id.clone()).await;
                            user_creds.insert(user_id.clone(), user.unwrap());
                            user_creds.get(&user_id).unwrap()
                        }
                        Some(u) => u,
                    };

                    dbg!(current_user_creds);

                    println!("SHOULD GET NOTION DATA FOR THIS USER");
                    let notion_data = current_user_creds.notion_data.as_ref().unwrap();
                    let notion_client = notion_api::NotionClientUnauthenticated::new();
                    let x = notion_client
                        .get_pages_from_notion_database(
                            &notion_data.notion_access_token,
                            "asdfasdf",
                        )
                        .await;
                    dbg!(x.unwrap());

                    println!(
                        "THEN GET GOOGLE CALENDAR RECENTLY EDITED STUFF (USING SYNC ENDPOINT?)"
                    );

                    println!("THEN COMPARE -> THIS IS THE KEY LOGIC");

                    println!("MAKE ANY REQUIRED CHANGES");

                    debug!("end of single sync pipeline");
                }
                .instrument(single_sync_job_span)
                .await;
            }

            tokio::time::sleep(Duration::from_secs(20))
                .instrument(debug_span!("artificial sleep time"))
                .await;

            anyhow::Ok(())
        };

        async {
            let result = sync_job.await;
            result.map_err(|e| {
                error!(error = %e, "error!!!!! (this is the one at the end)");
                e
            })
        }
        .instrument(pipeline_span)
        .await?;
        // .instrument(pipeline_span)
        // .await
        // .map_err(|e| {
        //     error!(error = %e, "error!!!!! (this is the one at the end)");
        //     e
        // })?;
    }
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
