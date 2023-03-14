# just manual: https://github.com/casey/just/blob/master/README.md
set dotenv-load

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
