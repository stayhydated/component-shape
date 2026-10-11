#!/usr/bin/env bash
set -euo pipefail

taplo fmt --check
rumdl check .
cargo sort-derives --check
typos
# actionlint also runs ShellCheck on inline workflow shell commands.
actionlint
shellcheck .github/scripts/*.sh
cargo fmt --manifest-path ci/feature-consumer/Cargo.toml -- --check
