//! Clustering management using etcd. Get the number of replicas and manage leases on sync
//! partitions.

use once_cell::sync::Lazy;
use thiserror::Error;
use tracing::{event, span, Instrument, Level};

use crate::do_with_retries;

use crate::etcd::{
    etcdserverpb::{PutResponse, RangeResponse},
    EtcdClients, KvClient,
};

pub const REPLICA_PREFIX: &str = "/nodes/";
pub const SYNC_LOCK_PREFIX: &str = "/sync_locks/";
pub static REPLICA_PREFIX_RANGE_END: Lazy<String> =
    Lazy::new(|| crate::etcd::calculate_prefix_range_end(REPLICA_PREFIX));

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error in etcd module")]
    EtcdError(#[from] crate::etcd::Error),
    #[error("Missing environment variable {0}")]
    EnvVar(String),
    #[error("Error recording node cluster membership")]
    RecordingMembershipError(#[from] tonic::Status),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Get an env var but record what its name was if it is missing
fn get_env_var(key: &str) -> Result<String> {
    std::env::var(key).or(Err(Error::EnvVar(key.into())))
}

/// Manage cluster membership recording
///
/// Uses [record_node_membership] and various lease functions.
///
/// Doesn't return a result, so that it can run nicely in a separate tokio task. Will just retry
/// the whole thing if any part fails.
pub async fn manage_cluster_node_membership(mut etcd_clients: EtcdClients) {
    loop {
        let mut lease = Default::default();
        let result = async {
            lease = do_with_retries(|| crate::etcd::create_lease(etcd_clients.lease.clone())).await;

            event!(
                Level::INFO,
                etcd_lease_id = lease.id,
                "current lease: {:#?}",
                lease.id
            );

            record_node_membership(&mut etcd_clients, lease.id)
                .await
                .map_err(|e| {
                    event!(Level::ERROR, "{:#?}", e);
                    e
                })?;

            Ok::<_, Error>(())
        }
        .instrument(span!(
            Level::INFO,
            "initialise lease for cluster membership"
        ))
        .await;

        match result {
            Ok(_) => {
                // TODO: take in the shutdown signal channel, then stop [etcd::lease_keep_alive] and switch
                // to a task that revokes the etcd lease for a clean shutdown.
                let result =
                    crate::etcd::lease_keep_alive(etcd_clients.lease.clone(), lease.id).await;

                if result.is_err() {
                    println!("Error with lease_keep_alive, will create a new lease")
                };
            }
            Err(e) => {
                event!(
                    Level::ERROR,
                    "Error initialising cluster membership, will try again. Error: {e:#?}"
                );
            }
        };

        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

/// Records node membership of the cluster of workers. This communicates with etcd and uses the
/// current hostname as an identifier.
#[tracing::instrument]
pub async fn record_node_membership(
    etcd_clients: &mut EtcdClients,
    lease: i64,
) -> Result<PutResponse> {
    let hostname = get_env_var("HOSTNAME")?;

    let kv_request = tonic::Request::new(crate::etcd::PutRequest {
        key: format!("{}{}", REPLICA_PREFIX, hostname).into(),
        lease,
        value: "replica".into(),
        ..Default::default()
    });

    Ok(etcd_clients.kv.put(kv_request).await?.into_inner())
}

/// Get a count of registered cluster workers/nodes
#[tracing::instrument]
pub async fn get_current_cluster_members_count(kv_client: &mut KvClient) -> Result<i64> {
    let range_end: String = REPLICA_PREFIX_RANGE_END.to_string();
    event!(
        Level::DEBUG,
        "request range: {} to {}",
        REPLICA_PREFIX,
        range_end
    );

    let range_request = tonic::Request::new(self::etcd::etcdserverpb::RangeRequest {
        key: REPLICA_PREFIX.into(),
        range_end: range_end.into(),
        count_only: true,
        ..Default::default()
    });

    Ok(kv_client.range(range_request).await?.into_inner().count)
}

/// Get all worker replica records from etcd
#[tracing::instrument]
pub async fn get_all_worker_records(kv_client: &mut KvClient) -> Result<RangeResponse> {
    let range_end: String = REPLICA_PREFIX_RANGE_END.to_string();
    event!(
        Level::DEBUG,
        "request range: {} to {}",
        REPLICA_PREFIX,
        range_end
    );

    let range_request = tonic::Request::new(self::etcd::etcdserverpb::RangeRequest {
        key: REPLICA_PREFIX.into(),
        range_end: range_end.into(),
        ..Default::default()
    });

    Ok(kv_client.range(range_request).await?.into_inner())
}
