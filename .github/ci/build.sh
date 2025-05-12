#!/bin/bash
set -euo pipefail

pushd firmware/loeti
cargo fmt
cargo clippy -- -D warnings
cargo build --release
popd
