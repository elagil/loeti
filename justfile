export FORMAT_ARGS := "--manifest-path firmware/loeti/Cargo.toml"
export BUILD_ARGS := "--manifest-path firmware/loeti/Cargo.toml --config firmware/loeti/.cargo/config.toml"

format:
    cargo +nightly fmt $FORMAT_ARGS --all

lint:
    cargo clippy $BUILD_ARGS -- -D warnings

build:
    cargo build $BUILD_ARGS --release

run:
    cargo run $BUILD_ARGS --release
