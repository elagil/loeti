export FIRMWARE_DIR_COMMON := "firmware/loeti/common"
export FIRMWARE_DIR_V6 := "firmware/loeti/board_v6"
export FIRMWARE_DIR_V7 := "firmware/loeti/board_v7"
export APP_DIR := "app"

style: format lint

format:
    cd $FIRMWARE_DIR_COMMON && cargo +nightly fmt --all
    cd $FIRMWARE_DIR_V6 && cargo +nightly fmt --all
    cd $FIRMWARE_DIR_V7 && cargo +nightly fmt --all
    cd $APP_DIR && cargo +nightly fmt --all

lint:
    cd $FIRMWARE_DIR_COMMON && cargo clippy -- -D warnings
    cd $FIRMWARE_DIR_V6 && cargo clippy -- -D warnings
    cd $FIRMWARE_DIR_V7 && cargo clippy -- -D warnings
    cd $APP_DIR && cargo clippy -- -D warnings

build:
    cd $FIRMWARE_DIR_COMMON && cargo build --release
    cd $FIRMWARE_DIR_V6 && cargo build --release
    cd $FIRMWARE_DIR_V7 && cargo build --release
    cd $APP_DIR && cargo build

release:
    cd $FIRMWARE_DIR_V6 && cargo objcopy --release --no-default-features --features comm -- -O binary loeti_board_v6_comm.bin
    cd $FIRMWARE_DIR_V6 && cargo objcopy --release -- -O binary loeti_board_v6.bin
    cd $FIRMWARE_DIR_V7 && cargo objcopy --release -- -O binary loeti_board_v7.bin
