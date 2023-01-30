# just manual: https://github.com/casey/just/blob/master/README.md

list:
    just --list

run:
    env $(cat .env | xargs) cargo run

fun:
    env $(cat .env | xargs) cargo run --bin fun

lint:
    cargo clippy
