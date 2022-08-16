set dotenv-load
set export

cargo := "~/.cargo/bin/cargo"

list:
    @just --list

build:
    @{{cargo}} build

run:
    @{{cargo}} run

test:
    @{{cargo}} test
