export PROJECT_DIR := "firmware/loeti"

format:
    pushd $PROJECT_DIR && \
    cargo +nightly fmt --all

lint:
    pushd $PROJECT_DIR && \
    cargo clippy -- -D warnings

build:
    pushd $PROJECT_DIR && \
    cargo build --release

run:
    pushd $PROJECT_DIR && \
    cargo run --release
