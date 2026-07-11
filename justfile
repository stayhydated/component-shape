set windows-shell := ["pwsh.exe", "-NoLogo", "-Command"]

default:
    @just --list

fmt:
    cargo sort-derives
    cargo fmt
    taplo fmt
    rumdl fmt .

clippy:
    cargo clippy --workspace --all-features --all-targets --locked -- -D warnings

check:
    cargo check --workspace --all-features --all-targets --locked

test:
    cargo test --workspace --all-features --locked

cov:
    cargo llvm-cov --workspace --all-features --all-targets

test-publish:
    cargo xtask release plan

test-docs:
    cargo doc --workspace --all-features --no-deps --locked --open
