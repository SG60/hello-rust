//! Clustering management using etcd. Get the number of replicas and manage leases on sync
//! partitions.

use once_cell::sync::Lazy;
use thiserror::Error;
use tracing::{event, Level};

use crate::{do_with_retries, etcd};

use crate::etcd::{
    etcdserverpb::{PutResponse, RangeResponse},
    EtcdClients, KvClient,
};

pub const REPLICA_PREFIX: &str = "/nodes/";
pub static REPLICA_PREFIX_RANGE_END: Lazy<String> =
    Lazy::new(|| crate::etcd::calculate_prefix_range_end(REPLICA_PREFIX));
pub const SYNC_LOCK_PREFIX: &str = "/sync_locks/";
pub static SYNC_LOCK_PREFIX_RANGE_END: Lazy<String> =
    Lazy::new(|| crate::etcd::calculate_prefix_range_end(SYNC_LOCK_PREFIX));

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

#[tracing::instrument]
pub async fn initialise_lease_and_node_membership(
    etcd_clients: EtcdClients,
    node_name: String,
) -> Result<etcd::LeaseGrantResponse> {
    let lease = do_with_retries(|| crate::etcd::create_lease(etcd_clients.lease.clone())).await;

    event!(
        Level::TRACE,
        etcd_lease_id = lease.id,
        "current lease: {:#?}",
        lease.id
    );

    record_node_membership(&mut etcd_clients.clone(), lease.id, node_name.clone())
        .await
        .map_err(|e| {
            event!(Level::ERROR, "{:#?}", e);
            e
        })?;

    Ok(lease)
}

/// Records node membership of the cluster of workers. This communicates with etcd and uses the
/// current hostname as an identifier.
#[tracing::instrument]
pub async fn record_node_membership(
    etcd_clients: &mut EtcdClients,
    lease: i64,
    node_name: String,
) -> Result<PutResponse> {
    let hostname = node_name;

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

    let range_request = tonic::Request::new(crate::etcd::etcdserverpb::RangeRequest {
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

    let range_request = tonic::Request::new(crate::etcd::etcdserverpb::RangeRequest {
        key: REPLICA_PREFIX.into(),
        range_end: range_end.into(),
        ..Default::default()
    });

    Ok(kv_client.range(range_request).await?.into_inner())
}

/// Get all lock partition records from etcd
#[tracing::instrument]
pub async fn get_all_sync_lock_records(kv_client: &mut KvClient) -> Result<RangeResponse> {
    let range_end: String = SYNC_LOCK_PREFIX_RANGE_END.to_string();

    let range_request = tonic::Request::new(crate::etcd::etcdserverpb::RangeRequest {
        key: SYNC_LOCK_PREFIX.into(),
        range_end: range_end.into(),
        ..Default::default()
    });

    Ok(kv_client.range(range_request).await?.into_inner())
}

/// Create a KV record in etcd to represent a worker lock for this worker
#[tracing::instrument]
pub async fn create_a_sync_lock_record(
    kv_client: &mut KvClient,
    current_lease: i64,
    worker_id: String,
    lock_key: &str,
) -> Result<()> {
    let lock_key: Vec<u8> = format!("{}{}", SYNC_LOCK_PREFIX, lock_key).into();
    kv_client
        .txn(etcd::TxnRequest {
            compare: vec![etcd::Compare {
                result: etcd::compare::CompareResult::Equal.into(),
                key: lock_key.clone(),
                range_end: lock_key.clone(),
                target: etcd::compare::CompareTarget::Version.into(),
                target_union: Some(etcd::compare::TargetUnion::Version(0)),
            }],
            success: vec![etcd::RequestOp {
                request: Some(etcd::request_op::Request::RequestPut(etcd::PutRequest {
                    key: lock_key,
                    value: worker_id.into(),
                    lease: current_lease,
                    prev_kv: false,
                    ignore_value: false,
                    ignore_lease: false,
                })),
            }],
            failure: vec![],
        })
        .await?;

    Ok(())
}

#[tracing::instrument]
pub async fn create_n_sync_lock_records(
    kv_client: &mut KvClient,
    current_lease: i64,
    worker_id: String,
    number_of_sync_partitions: usize,
    workers_count: usize,
    current_worker_index: usize,
) -> Result<()> {
    let sync_records_to_claim: Vec<usize> = ((current_worker_index)..number_of_sync_partitions)
        .step_by(workers_count)
        .collect();

    dbg!(&sync_records_to_claim);

    let n_sync_records_to_claim = sync_records_to_claim.len();

    for i in sync_records_to_claim {
        create_a_sync_lock_record(
            kv_client,
            current_lease,
            worker_id.to_owned(),
            &i.to_string(),
        )
        .await?;
    }

    event!(
        Level::DEBUG,
        workers_count,
        worker_id,
        current_worker_index,
        n_sync_records_to_claim
    );

    Ok(())
}

#[tracing::instrument]
pub async fn get_worker_records_and_establish_locks(
    kv_client: &mut KvClient,
    node_name: &str,
    current_lease: i64,
) -> Vec<u16> {
    let list = get_all_worker_records(kv_client).await;
    if let Ok(list) = list {
        let mapped_kv: Vec<_> = list
            .kvs
            .iter()
            .map(|element| {
                std::str::from_utf8(&element.key)
                    .expect("Should be valid utf8")
                    .strip_prefix(REPLICA_PREFIX)
                    .expect("should be formatted with /nodes/ at start")
            })
            .collect();

        let current_worker_index = mapped_kv
            .iter()
            .position(|x| *x == node_name)
            .expect("should exist");
        let workers_count = list.count;

        // This should be equal to the total number of sync partitions in DynamoDB.
        // Perhaps there should be a way to calculate this automatically?! For now it
        // is fine as a compile time constant.
        let total_number_of_sync_partitions = 21;

        create_n_sync_lock_records(
            kv_client,
            current_lease,
            node_name.to_string(),
            total_number_of_sync_partitions,
            workers_count.try_into().unwrap(),
            current_worker_index,
        )
        .await
        .unwrap();

        let current_lock_records = get_all_sync_lock_records(kv_client)
            .await
            .expect("should be valid");
        let sync_partitions: Vec<_> = current_lock_records
            .kvs
            .iter()
            .filter_map(|element| {
                if std::str::from_utf8(&element.value).expect("Should be valid utf8") == node_name {
                    Some(
                        std::str::from_utf8(&element.key)
                            .expect("Should be valid utf8")
                            .strip_prefix(SYNC_LOCK_PREFIX)
                            .expect("should be formatted with correct prefix")
                            .parse()
                            .expect("should be valid number"),
                    )
                } else {
                    None
                }
            })
            .collect();
        dbg!(sync_partitions.clone());

        event!(
            Level::DEBUG,
            workers_count,
            node_name,
            current_lease,
            current_worker_index,
            "kvs strings: {:#?}",
            mapped_kv
        );

        sync_partitions
    } else {
        vec![]
    }
}
