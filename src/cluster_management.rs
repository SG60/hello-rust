//! Clustering management using etcd. Get the number of replicas and manage leases on sync
//! partitions.

use once_cell::sync::Lazy;
use thiserror::Error;
use tracing::{event, span, Instrument, Level};

use crate::{do_with_retries, etcd, loop_getting_cluster_members};

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

/// Manage cluster membership recording
///
/// Uses [record_node_membership] and various lease functions.
///
/// Doesn't return a result, so that it can run nicely in a separate tokio task. Will just retry
/// the whole thing if any part fails.
pub async fn manage_cluster_node_membership(mut etcd_clients: EtcdClients, node_name: String) {
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

            record_node_membership(&mut etcd_clients.clone(), lease.id, node_name.clone())
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

                let lease_keep_alive_join_handle = tokio::spawn(crate::etcd::lease_keep_alive(
                    etcd_clients.clone().lease,
                    lease.id,
                ));
                let run_work_join_handle = tokio::spawn(loop_getting_cluster_members(
                    etcd_clients.clone(),
                    node_name.clone(),
                    lease.id,
                ));

                tokio::select! {
                    handle = lease_keep_alive_join_handle => {
                        let result = handle.unwrap();
                        dbg!("lease_keep_alive_join_handle completed!");

                        if result.is_err() {
                            println!("Error with lease_keep_alive, will create a new lease")
                        };
                    },
                    _ = run_work_join_handle => {
                        dbg!("run_work_join_handle completed!");
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

/// Create a KV record in etcd to represent a worker lock for this worker
#[tracing::instrument]
pub async fn create_a_sync_lock_record(
    kv_client: &mut KvClient,
    current_lease: i64,
    worker_id: String,
    lock_key: &str,
) -> Result<()> {
    // let result = kv_client
    //     .put(etcd::PutRequest {
    //         key: lock_key.into(),
    //         value: worker_id.clone().into(),
    //         lease: current_lease,
    //         prev_kv: false,
    //         ignore_value: false,
    //         ignore_lease: false,
    //     })
    //     .await?;
    //
    // dbg!(result);

    let result = kv_client
        .txn(etcd::TxnRequest {
            compare: vec![etcd::Compare {
                result: etcd::compare::CompareResult::Equal.into(),
                key: lock_key.into(),
                range_end: lock_key.into(),
                target: etcd::compare::CompareTarget::Version.into(),
                target_union: Some(etcd::compare::TargetUnion::Version(0)),
            }],
            success: vec![etcd::RequestOp {
                request: Some(etcd::request_op::Request::RequestPut(etcd::PutRequest {
                    key: lock_key.into(),
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

    dbg!(result);

    unimplemented!()
}

#[tracing::instrument]
pub async fn create_n_sync_lock_records(
    kv_client: &mut KvClient,
    current_lease: i64,
    worker_id: String,
    number_of_sync_partitions: usize,
    workers_count: usize,
    current_worker_index: usize,
) {
    let sync_records_to_claim: Vec<usize> = ((current_worker_index)..number_of_sync_partitions)
        .step_by(workers_count)
        .collect();

    dbg!(&sync_records_to_claim);

    let n_sync_records_to_claim = sync_records_to_claim.len();

    // TODO: THIS NEEDS THE LEASE SOMEHOW. MAYBE THERE SHOULD BE A CHANNEL FROM
    // THE CLUSTER MANAGEMENT THREAD TO THE WORKER STUFF, WOULD BOTH TELL THE
    // WORKER BIT WHEN TO REFRESH (E.G. HAD TO GET A NEW LEASE) AND PROVIDE THE
    // UP-TO-DATE LEASE_ID. OR MAYBE THIS SHOULD ALL BE IN THAT THREAD AS WELL
    // ANYWAY?!?!??!
    let result = for i in sync_records_to_claim {
        create_a_sync_lock_record(
            kv_client,
            current_lease,
            worker_id.to_owned(),
            &i.to_string(),
        )
        .await;
    };

    dbg!(result);

    event!(
        Level::DEBUG,
        workers_count,
        worker_id,
        current_worker_index,
        n_sync_records_to_claim
    );

    result
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
                    .strip_prefix("/nodes/")
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

        let result = create_n_sync_lock_records(
            kv_client,
            current_lease,
            node_name.to_string(),
            total_number_of_sync_partitions,
            workers_count.try_into().unwrap(),
            current_worker_index,
        )
        .await;

        dbg!(result);

        let sync_partitions = vec![1, 2, 3, 4];

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
