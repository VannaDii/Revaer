set shell := ["bash", "-lc"]

default:
    @just --list

fmt:
    @cargo fmt --all --check

fmt-fix:
    @cargo fmt --all

lint:
    @RUSTFLAGS="-Dwarnings" cargo clippy --workspace --all-targets --all-features -- -Dclippy::all -Dclippy::pedantic -Dclippy::cargo -Dclippy::nursery -Aclippy::multiple_crate_versions

check:
    @RUSTFLAGS="-Dwarnings" cargo check --workspace --all-features

test:
    @RUSTFLAGS="-Dwarnings" cargo test --workspace --all-features

build:
    @cargo build --workspace --all-features

build-rel:
    @cargo build --workspace --release --all-features

udeps:
    @cargo +stable udeps --workspace --all-targets

audit:
    @cargo audit

deny:
    @cargo deny check

cov:
    @cargo llvm-cov --workspace --fail-under 80

api-export:
    @cargo run -p revaer-api --bin generate_openapi

ci:
    @just fmt lint udeps audit deny test cov
