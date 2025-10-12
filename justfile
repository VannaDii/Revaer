default:
    @just --list

fmt:
    cargo fmt

lint:
    cargo clippy --all-targets --all-features -- -D warnings

test:
    cargo test

check:
    cargo check --all-targets --all-features
