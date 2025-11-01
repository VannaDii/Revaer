set shell := ["bash", "-lc"]

fmt:
    cargo fmt --all --check

fmt-fix:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

check:
    cargo --config 'build.rustflags=["-Dwarnings"]' check --workspace --all-targets --all-features

test:
    cargo --config 'build.rustflags=["-Dwarnings"]' test --workspace --all-features

build:
    cargo build --workspace --all-features

build-release:
    cargo build --workspace --release --all-features

udeps:
    if ! command -v cargo-udeps >/dev/null 2>&1; then \
        cargo install cargo-udeps --locked; \
    fi
    cargo +stable udeps --workspace --all-targets

audit:
    if ! command -v cargo-audit >/dev/null 2>&1; then \
        cargo install cargo-audit --locked; \
    fi
    cargo audit --deny warnings --ignore-file .secignore

deny:
    if ! command -v cargo-deny >/dev/null 2>&1; then \
        cargo install cargo-deny --locked; \
    fi
    cargo deny check

cov:
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then \
        cargo install cargo-llvm-cov --locked; \
    fi
    rustup component add llvm-tools-preview
    cargo llvm-cov --workspace --fail-under 80

api-export:
    cargo run -p revaer-api --bin generate_openapi

ci:
    just fmt lint udeps audit deny test cov

docker-build:
    docker build --tag revaer:ci .

docker-scan:
    if ! command -v trivy >/dev/null 2>&1; then \
        echo "trivy not installed; install it to run this scan" >&2; \
        exit 1; \
    fi
    trivy image --exit-code 1 --severity HIGH,CRITICAL revaer:ci

install-docs:
    if ! command -v mdbook >/dev/null 2>&1; then \
        cargo install --locked mdbook; \
    fi
    if ! command -v mdbook-mermaid >/dev/null 2>&1; then \
        cargo install --locked mdbook-mermaid; \
    fi
    mdbook-mermaid install .
    mkdir -p scripts
    mv -f mermaid*.js scripts/ 2>/dev/null || true

docs-build:
    mdbook build

docs-serve:
    mdbook serve --open

docs-index:
    cargo run -p revaer-doc-indexer --release

docs:
    just docs-build
    just docs-index
