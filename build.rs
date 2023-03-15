fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().build_server(false).compile(
        &[
            "etcd-api-protos/etcd/api/etcdserverpb/rpc.proto",
            "etcd-api-protos/etcd/api/authpb/auth.proto",
            "etcd-api-protos/etcd/api/mvccpb/kv.proto",
        ],
        &["etcd-api-protos"],
    )?;
    Ok(())
}
