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
    @rustup toolchain install nightly --profile minimal --no-self-update >/dev/null 2>&1 || true
    @if ! command -v cargo-udeps >/dev/null 2>&1; then \
        cargo +nightly install cargo-udeps --locked; \
    fi
    @cargo +nightly udeps --workspace --all-targets

audit:
    @if ! command -v cargo-audit >/dev/null 2>&1; then \
        cargo install cargo-audit --locked; \
    fi
    @cargo audit --deny warnings

deny:
    @if ! command -v cargo-deny >/dev/null 2>&1; then \
        cargo install cargo-deny --locked; \
    fi
    @cargo deny check

cov:
    @if ! command -v cargo-llvm-cov >/dev/null 2>&1; then \
        cargo install cargo-llvm-cov --locked; \
    fi
    @rustup component add llvm-tools-preview
    @cargo llvm-cov --workspace --fail-under-lines 80 --fail-under-functions 80 --fail-under-regions 80

api-export:
    @cargo run -p revaer-api --bin generate_openapi

ci:
    @just fmt lint udeps audit deny test cov
