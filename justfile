set dotenv-load
set export

list:
    @just --list

build:
    @cargo build

run:
    @cargo run

test:
    @cargo test
