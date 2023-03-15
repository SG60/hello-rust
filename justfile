# just manual: https://github.com/casey/just/blob/master/README.md
set dotenv-load

default:
  @just --choose

list:
    just --list

run:
    env $(cat .env | xargs) cargo run

# run the `fun` binary
fun:
    env $(cat .env | xargs) cargo run --bin fun

# clippy
lint:
    cargo clippy

# watch (default to running test)
watch +COMMAND='test':
    cargo watch --clear --exec "{{COMMAND}}"

test *FLAGS:
  cargo test -- {{FLAGS}}

test-stdout *FLAGS:
  just test --show-output {{FLAGS}}

test-all *FLAGS:
  just test --include-ignored {{FLAGS}}

# build for arm64
build-arm64:
  cross build --target=aarch64-unknown-linux-gnu --release

run-with-jaeger:
  #!/bin/bash
  set -ux
  trap '' INT
  run_commands () (
     trap - INT
     echo 'starting jaeger'
     docker run -d --name jaeger -p 4317:4317 -p 16686:16686 -e COLLECTOR_OTLP_ENABLED=true jaegertracing/all-in-one:latest
     echo 'just run'
     just run
  )
  run_commands
  docker stop jaeger; docker rm -v jaeger
