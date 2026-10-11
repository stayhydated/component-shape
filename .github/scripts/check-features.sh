#!/usr/bin/env bash
set -euo pipefail

# Each invocation has its own feature graph, outside the repository workspace.
export CARGO_TARGET_DIR="$PWD/target/feature-consumer"
for feature in mcp-minimal mcp-default mcp-chrono mcp-decimal; do
    cargo run --manifest-path ci/feature-consumer/Cargo.toml --locked \
        --no-default-features --features "$feature"
done
