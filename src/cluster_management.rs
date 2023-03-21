//! Clustering management using etcd. Get the number of replicas and manage leases on sync
//! partitions.

use thiserror::Error;

use self::etcd::{
    etcdserverpb::{PutResponse, RangeResponse},
    EtcdClients, KvClient,
};

pub const REPLICA_PREFIX: &str = "/nodes/";
pub const SYNC_LOCK_PREFIX: &str = "/sync_locks/";

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

/// Get all worker replica records from etcd
#[tracing::instrument]
pub async fn get_all_worker_records(kv_client: &mut KvClient) -> Result<RangeResponse> {
    let range_request = tonic::Request::new(self::etcd::etcdserverpb::RangeRequest {
        key: REPLICA_PREFIX.into(),
        ..Default::default()
    });

    Ok(kv_client.range(range_request).await?.into_inner())
}

/// Etcd grpc api
pub mod etcd {
    // reexports
    pub use self::etcdserverpb::{
        kv_client, lease_client, LeaseGrantRequest, LeaseGrantResponse, LeaseKeepAliveRequest,
        PutRequest,
    };

    use std::env::VarError;
    use thiserror::Error;
    use tokio::sync::mpsc::channel;
    use tokio::sync::mpsc::Sender;
    use tokio_stream::wrappers::ReceiverStream;
    use tonic::transport::Endpoint;
    use tracing::{event, Level};

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

    #[tracing::instrument]
    pub async fn lease_keep_alive(
        mut lease_client: LeaseClient,
        lease_id: i64,
    ) -> Result<LeaseKeepAlive> {
        event!(Level::INFO, "trying to keep the lease alive");

        let (req_sender, req_receiver) = channel(1024);
        let req_receiver = ReceiverStream::new(req_receiver);

        let initial_lease_request = LeaseKeepAliveRequest { id: lease_id };

        event!(Level::INFO, "lease_id: {}", lease_id);

        req_sender
            .send(initial_lease_request)
            .await
            .map_err(|_| Error::ChannelClosed)?;

        let mut response_receiver: tonic::Streaming<etcdserverpb::LeaseKeepAliveResponse> =
            lease_client
                .lease_keep_alive(req_receiver)
                .await?
                .into_inner();

        let lease_id = match response_receiver.message().await? {
            Some(resp) => resp.id,
            None => {
                return Err(Error::CreateWatch);
            }
        };

        Ok(LeaseKeepAlive {
            id: lease_id,
            request_sender: req_sender,
            response_stream: response_receiver,
        })
    }
}
