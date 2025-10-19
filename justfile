export PROJECT_DIR := "firmware/loeti"

style: format lint

format:
    cd $PROJECT_DIR && \
    cargo +nightly fmt --all

lint:
    cd $PROJECT_DIR && \
    cargo clippy -- -D warnings

build:
    cd $PROJECT_DIR && \
    cargo build --release

run:
    cd $PROJECT_DIR && \
    cargo run --release
