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
    cargo llvm-cov --workspace --exclude xtask --exclude web --all-features --all-targets

test-publish:
    cargo xtask release plan

test-docs:
    cargo doc --workspace --all-features --no-deps --locked --open

book:
    mdbook serve book

web-build:
    cargo xtask build book
    cargo xtask build llms-txt
    cargo xtask build web

web: web-build
    dx serve --package web

web-preview: web-build
    cargo xtask preview web
