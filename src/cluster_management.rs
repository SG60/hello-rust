//! Clustering management using etcd. Get the number of replicas and manage leases on sync
//! partitions.

use once_cell::sync::Lazy;
use thiserror::Error;
use tracing::{event, Level};

use self::etcd::{
    etcdserverpb::{PutResponse, RangeResponse},
    EtcdClients, KvClient,
};

pub const REPLICA_PREFIX: &str = "/nodes/";
pub const SYNC_LOCK_PREFIX: &str = "/sync_locks/";
pub static REPLICA_PREFIX_RANGE_END: Lazy<String> =
    Lazy::new(|| self::etcd::calculate_prefix_range_end(REPLICA_PREFIX));

#[derive(Error, Debug)]
pub enum Error {
    #[error("Error in etcd module")]
    EtcdError(#[from] self::etcd::Error),
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

/// Records node membership of the cluster of workers. This communicates with etcd and uses the
/// current hostname as an identifier.
#[tracing::instrument]
pub async fn record_node_membership(
    etcd_clients: &mut EtcdClients,
    lease: i64,
) -> Result<PutResponse> {
    let hostname = get_env_var("HOSTNAME")?;

    let kv_request = tonic::Request::new(self::etcd::PutRequest {
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

    event!(Level::DEBUG, "range_request: {:#?}", range_request);

    Ok(kv_client.range(range_request).await?.into_inner())
}

/// Etcd grpc api
pub mod etcd {
    use self::etcdserverpb::LeaseKeepAliveResponse;
    // reexports
    pub use self::etcdserverpb::{
        kv_client, lease_client, LeaseGrantRequest, LeaseGrantResponse, LeaseKeepAliveRequest,
        PutRequest,
    };

    use std::env::VarError;
    use std::time::Duration;
    use thiserror::Error;
    use tokio::sync::mpsc::channel;
    use tokio::sync::mpsc::Sender;
    use tokio::time::Instant;
    use tokio_stream::wrappers::ReceiverStream;
    use tonic::transport::Endpoint;
    use tonic::Streaming;
    use tracing::{event, span, Instrument, Level};

    use crate::tracing_utils::GrpcInterceptor;
    use crate::tracing_utils::InterceptedGrpcService;

    #[allow(clippy::all)]
    pub mod mvccpb {
        tonic::include_proto!("mvccpb"); // The string specified here must match the proto package name
    }

    #[allow(clippy::all)]
    pub mod authpb {
        tonic::include_proto!("authpb");
    }
    #[allow(clippy::all)]
    pub mod etcdserverpb {
        tonic::include_proto!("etcdserverpb");
    }

    #[derive(Debug)]
    pub struct LeaseKeepAlive {
        pub id: i64,
        pub request_sender: Sender<etcdserverpb::LeaseKeepAliveRequest>,
        pub response_stream: tonic::Streaming<etcdserverpb::LeaseKeepAliveResponse>,
    }

    pub type Result<T> = std::result::Result<T, Error>;
    #[derive(Error, Debug)]
    pub enum Error {
        #[error("environment variable not found")]
        VarError(#[from] VarError),
        #[error("error in grpc response status")]
        ResponseStatusError(#[from] tonic::Status),
        #[error("channel closed")]
        ChannelClosed,
        #[error("gRPC transport error")]
        Transport(#[from] tonic::transport::Error),
        #[error("failed to create watch")]
        CreateWatch,
        #[error("error refreshing lease")]
        RefreshLease,
    }

    pub type KvClient = kv_client::KvClient<InterceptedGrpcService>;
    pub type LeaseClient = lease_client::LeaseClient<InterceptedGrpcService>;
    #[derive(Debug)]
    pub struct EtcdClients {
        pub kv: KvClient,
        pub lease: LeaseClient,
    }
    impl EtcdClients {
        pub async fn connect(etcd_endpoint: String) -> Result<Self> {
            let channel = Endpoint::from_shared(etcd_endpoint)?.connect().await?;
            Ok(Self {
                kv: kv_client::KvClient::with_interceptor(channel.clone(), GrpcInterceptor),
                lease: lease_client::LeaseClient::with_interceptor(channel, GrpcInterceptor),
            })
        }
    }

    #[tracing::instrument]
    pub async fn create_lease(mut grpc_client: LeaseClient) -> Result<LeaseGrantResponse> {
        let request = tonic::Request::new(LeaseGrantRequest { id: 0, ttl: 30 });
        let response = grpc_client.lease_grant(request).await?;

        event!(Level::INFO, "Response={:?}", response);

        Ok(response.into_inner())
    }

    struct RefreshLeaseOnceResponse {
        ttl_in_seconds: i64,
    }
    #[tracing::instrument(level = "debug")]
    async fn refresh_lease_once(
        request_sender: &Sender<LeaseKeepAliveRequest>,
        response_receiver: &mut tonic::Streaming<LeaseKeepAliveResponse>,
        lease_id: i64,
    ) -> Result<RefreshLeaseOnceResponse> {
        send_lease_keep_alive_request(&request_sender, lease_id).await?;

        event!(Level::INFO, lease_id, "trying to keep the lease alive");

        let lease_ttl;
        // wait for response on channel (confirming the lease refresh)
        if let Some(response) = response_receiver.message().await? {
            event!(Level::INFO, lease_ttl = response.ttl, "refreshed lease");
            lease_ttl = response.ttl;
        } else {
            return Err(Error::RefreshLease);
        };
        Ok(RefreshLeaseOnceResponse {
            ttl_in_seconds: lease_ttl,
        })
    }

    #[tracing::instrument(level = "debug")]
    async fn send_lease_keep_alive_request(
        request_sender: &Sender<LeaseKeepAliveRequest>,
        lease_id: i64,
    ) -> Result<()> {
        request_sender
            .send(LeaseKeepAliveRequest { id: lease_id })
            .await
            .map_err(|_| Error::ChannelClosed)
    }

    struct LeaseLivenessKeeper {
        request_sender: Sender<LeaseKeepAliveRequest>,
        response_receiver: Streaming<LeaseKeepAliveResponse>,
        lease_id: i64,
    }
    impl LeaseLivenessKeeper {
        /// send a keep alive request to etcd
        async fn keep_alive(&mut self) -> Result<RefreshLeaseOnceResponse> {
            refresh_lease_once(
                &self.request_sender,
                &mut self.response_receiver,
                self.lease_id,
            )
            .await
        }

        #[tracing::instrument]
        async fn initialise_lease_keep_alive(
            mut lease_client: LeaseClient,
            lease_id: i64,
        ) -> Result<LeaseLivenessKeeper> {
            event!(Level::DEBUG, "creating channel and ReceiverStream");
            let (req_sender, req_receiver) = channel::<LeaseKeepAliveRequest>(1024);
            send_lease_keep_alive_request(&req_sender, lease_id).await?;

            let req_receiver = ReceiverStream::new(req_receiver);

            println!("______________________let response_receiver_________________");
            let response_receiver_result = lease_client
                .lease_keep_alive(req_receiver)
                .instrument(span!(
                    Level::DEBUG,
                    "set up response receiver for lease refreshing"
                ))
                .await;
            println!("_______________________response_receiver_future got!_______________");
            dbg!(&response_receiver_result);
            let response_receiver: tonic::Streaming<etcdserverpb::LeaseKeepAliveResponse> =
                response_receiver_result?.into_inner();

            Ok(LeaseLivenessKeeper {
                lease_id,
                request_sender: req_sender,
                response_receiver,
            })
        }
    }

    pub async fn lease_keep_alive(
        lease_client: LeaseClient,
        lease_id: i64,
    ) -> Result<LeaseKeepAlive> {
        println!("______________________Keep the lease alive!!!_________________");

        let mut lease_liveness_keeper =
            LeaseLivenessKeeper::initialise_lease_keep_alive(lease_client, lease_id).await?;

        let ttl_desired_preemption = 10;
        let span = span!(Level::TRACE, "test spannnnn");
        let _enter = span.enter();

        println!("______________________just before the lease loop starts_________________");

        loop {
            async {
                let instant_before_request = Instant::now();

                let RefreshLeaseOnceResponse { ttl_in_seconds } =
                    lease_liveness_keeper.keep_alive().await?;

                let time_to_wait_before_renewal = if ttl_in_seconds <= ttl_desired_preemption {
                    ttl_in_seconds / 2
                } else {
                    ttl_in_seconds - ttl_desired_preemption
                };

                event!(
                    Level::INFO,
                    lease_ttl = ttl_in_seconds,
                    time_to_wait_before_renewal,
                    lease_id,
                    "lease renewal details"
                );

                println!("_______________________________________");
                println!("sleeping until next lease refresh: {time_to_wait_before_renewal}");

                tokio::time::sleep_until(
                    instant_before_request
                        + Duration::from_secs(
                            time_to_wait_before_renewal
                                .try_into()
                                .expect("should be a positive integer"),
                        ),
                )
                .instrument(span!(Level::DEBUG, "sleep"))
                .await;
                Ok::<_, Error>(())
            }
            .instrument(span!(Level::INFO, "refresh lease"))
            .await?;
        }
    }

    /// Calculate the correct range_end prefix (prefix + 1)
    pub fn calculate_prefix_range_end(prefix: &str) -> String {
        let mut calculated_prefix = prefix.to_string();
        let prefix_last_char = calculated_prefix.pop().expect("Should be a last char");
        let incremented_char = prefix_last_char as u8 + 1;
        calculated_prefix.push(incremented_char.into());

        calculated_prefix
    }
}
