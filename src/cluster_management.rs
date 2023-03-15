use std::env::VarError;
use thiserror::Error;

use tokio::sync::mpsc::{channel, Sender};
use tokio_stream::wrappers::ReceiverStream;
use tonic::transport::Channel;
use tracing::{event, Level};

use self::etcdserverpb::{
    kv_client::KvClient, lease_client::LeaseClient, LeaseGrantRequest, LeaseGrantResponse,
    LeaseKeepAliveRequest, PutRequest, PutResponse,
};

const REPLICA_PREFIX: &str = "/nodes/";
const SYNC_LOCK_PREFIX: &str = "/sync_locks/";

pub mod mvccpb {
    tonic::include_proto!("mvccpb"); // The string specified here must match the proto package name
}

pub mod authpb {
    tonic::include_proto!("authpb");
}

pub mod etcdserverpb {
    tonic::include_proto!("etcdserverpb");
}

pub async fn make_kv_client(etcd_endpoint: String) -> Result<KvClient<Channel>> {
    Ok(KvClient::connect(etcd_endpoint).await?)
}

pub async fn make_lease_client(etcd_endpoint: String) -> Result<LeaseClient<Channel>> {
    Ok(LeaseClient::connect(etcd_endpoint).await?)
}

#[tracing::instrument]
pub async fn create_lease(mut grpc_client: LeaseClient<Channel>) -> Result<LeaseGrantResponse> {
    let request = tonic::Request::new(LeaseGrantRequest { id: 0, ttl: 30 });
    let response = grpc_client.lease_grant(request).await?;

    event!(Level::INFO, "Response={:?}", response);

    Ok(response.into_inner())
}

#[derive(Error, Debug)]
enum Error {
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

pub type Result<T> = std::result::Result<T, Error>;

pub struct LeaseKeepAlive {
    id: i64,
    request_sender: Sender<etcdserverpb::LeaseKeepAliveRequest>,
    response_stream: tonic::Streaming<etcdserverpb::LeaseKeepAliveResponse>,
}

pub async fn lease_keep_alive(
    mut lease_client: LeaseClient<Channel>,
    lease_id: i64,
) -> Result<LeaseKeepAlive> {
    let hostname = std::env::var("HOST")?;

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

    // let request = tonic::Request::new(PutRequest {
    //     key: format!("{}{}", REPLICA_PREFIX, hostname).into(),
    //     value: "replica".into(),
    //     ..Default::default()
    // });
    //
    // let response = kv_client.put(request).await?;
    //
    // Ok(response.into_inner())
}
