# just manual: https://github.com/casey/just/blob/master/README.md

set dotenv-load := true

default:
    @just --list

j:
    just --choose

run:
    cargo run

# clippy
lint:
    cargo clippy && cargo fmt --check --all

# watch (default to running test)
watch COMMAND='test':
    cargo watch --clear --exec {{ COMMAND }}

test *FLAGS:
    cargo test -- {{ FLAGS }}

test-stdout *FLAGS:
    just test --show-output {{ FLAGS }}

test-all *FLAGS:
    just test --include-ignored {{ FLAGS }}

# build for arm64
build-arm64:
    cross build --target=aarch64-unknown-linux-gnu --release

jaeger:
    docker run --name jaeger -p 4317:4317 -p 16686:16686 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest

# Fetch the protobuf files for the etcd API
# version required (e.g. 3.5.7)

# Will still require some tweaking after the download
fetch-etcd-protobuf-files +VERSION:
    mkdir etcd-api-protos/etcd-repo
    curl -L https://github.com/etcd-io/etcd/archive/refs/tags/v{{ VERSION }}.tar.gz | tar -xvzf - -C etcd-api-protos/etcd-repo --strip-components=1
    rsync -am --include='*.proto' --include='*/' --exclude='*' etcd-api-protos/etcd-repo/api/ etcd-api-protos/etcd/api/
    rm -r etcd-api-protos/etcd-repo

# Run an etcd container
etcd $NODE1="192.168.1.101":
    echo $NODE1
    docker run \
      -p 2379:2379 \
      -p 2380:2380 \
      --name etcd quay.io/coreos/etcd:v3.5.9 \
      /usr/local/bin/etcd \
      --data-dir=/etcd-data --name node1 \
      --initial-advertise-peer-urls http://${NODE1}:2380 --listen-peer-urls http://0.0.0.0:2380 \
      --advertise-client-urls http://${NODE1}:2379 --listen-client-urls http://0.0.0.0:2379 \
      --initial-cluster node1=http://${NODE1}:2380

# Run a just command and then clean up the docker image of the same name
docker-with-cleanup just-cmd-and-container-name="etcd":
    #!/bin/bash
    set -ux
    trap '' INT
    run_commands () (
      trap - INT
      just {{ just-cmd-and-container-name }}
    )
    run_commands
    set +x
    echo "--- CLEANING UP ---"
    set -x
    docker stop {{ just-cmd-and-container-name }}; docker rm -v {{ just-cmd-and-container-name }}

backend_etcd_related_env := "HOSTNAME=" + uuid() + " APP_ETCD_URL=http://localhost:2379"

# Run against local etcd
run-with-etcd:
    {{ backend_etcd_related_env }} just run

export RUST_LOG := "hello_rust_backend=debug"
export NO_OTLP := "1"

dev:
    {{ backend_etcd_related_env }} cargo watch -x run

run-with-etcd-and-otlp $NO_OTLP="0":
    {{ backend_etcd_related_env }} cargo run

run-for-tokio-console $RUSTFLAGS="--cfg tokio_unstable" $NO_OTLP="1":
    {{ backend_etcd_related_env }} cargo run --features=tokio-console

tokio-console:
  tokio-console

# Cross compile using nix
cross-build nix-target="aarch64-multiplatform" cargo-target="aarch64-unknown-linux-gnu" profile="release":
  nix develop '.#pkgsCross.{{nix-target}}.hello-rust' -c cargo build --target {{cargo-target}} --profile {{profile}}

