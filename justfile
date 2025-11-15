export FIRMWARE_DIR := "firmware/loeti"
export APP_DIR := "app"

style: format lint

format:
    cd $FIRMWARE_DIR && \
    cargo +nightly fmt --all
    cd $APP_DIR && \
    cargo +nightly fmt --all

lint:
    cd $FIRMWARE_DIR && \
    cargo clippy -- -D warnings
    cd $APP_DIR && \
    cargo clippy -- -D warnings

build:
    cd $FIRMWARE_DIR && \
    cargo build --release
    cd $APP_DIR && \
    cargo build
