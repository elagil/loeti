#!/bin/bash
set -euo pipefail

for dir in firmware/loeti;
do
    pushd $dir
    cargo fmt
    cargo clippy -- -D warnings
    cargo build --release
    popd
done

for dir in firmware/loeti
do
    pushd $dir
    cargo clippy -- -D warnings
    popd
done
