# just manual: https://github.com/casey/just/blob/master/README.md
set dotenv-load

list:
    just --list

run:
    env $(cat .env | xargs) cargo run

fun:
    env $(cat .env | xargs) cargo run --bin fun

lint:
    cargo clippy

watch +COMMAND='test':
    cargo watch --clear --exec "{{COMMAND}}"

test *FLAGS:
  cargo test -- {{FLAGS}}

test-stdout *FLAGS:
  just test --show-output {{FLAGS}}

test-all *FLAGS:
  just test --include-ignored {{FLAGS}}
