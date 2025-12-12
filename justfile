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
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test --workspace --all-features

test-features-min:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-api --no-default-features
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-app --no-default-features

build:
    cargo build --workspace --all-targets --all-features

build-release:
    cargo build --workspace --release --all-targets --all-features

udeps:
    if ! command -v cargo-udeps >/dev/null 2>&1; then \
        cargo install cargo-udeps --locked; \
    fi
    if ! cargo +stable udeps --workspace --all-targets >/dev/null 2>&1; then \
        echo "cargo-udeps: stable toolchain lacks required -Z flags, retrying with nightly"; \
        if ! rustup toolchain list | grep -q nightly; then \
            rustup toolchain install nightly --no-self-update; \
        fi; \
        cargo +nightly udeps --workspace --all-targets; \
    fi

audit:
    if ! command -v cargo-audit >/dev/null 2>&1; then \
        cargo install cargo-audit --locked; \
    fi
    ignore_args=""; \
    if [ -f .secignore ]; then \
        while IFS= read -r advisory; do \
            case "$advisory" in \
                \#*|"") ;; \
                *) ignore_args="$ignore_args --ignore $advisory" ;; \
            esac; \
        done < .secignore; \
    fi; \
    cargo audit --deny warnings $ignore_args

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
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo llvm-cov --workspace --fail-under-lines 80

sbom:
    mkdir -p artifacts
    cargo metadata --format-version 1 --all-features --locked > artifacts/sbom.json

licenses:
    if ! command -v cargo-deny >/dev/null 2>&1; then \
        cargo install cargo-deny --locked; \
    fi
    mkdir -p artifacts
    cargo deny list --format json > artifacts/licenses.json

api-export:
    cargo run -p revaer-api --bin generate_openapi

ci:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}"
    export REVAER_TEST_DATABASE_URL DATABASE_URL
    just db-start
    just fmt lint udeps audit deny test test-features-min cov

docker-build:
    platforms="${PLATFORMS:-linux/amd64,linux/arm64}"; \
    version="${VERSION:-dev.$(date -u +%y%m%d).$(git rev-parse --short HEAD)}"; \
    tags="--tag revaer:latest --tag revaer:${version}"; \
    builder="${BUILDX_BUILDER:-revaer-builder}"; \
    if ! docker buildx inspect "$builder" >/dev/null 2>&1; then \
        docker buildx create --name "$builder" --driver docker-container --use; \
    else \
        docker buildx use "$builder"; \
    fi; \
    if printf "%s" "$platforms" | grep -q ','; then \
        mkdir -p artifacts; \
        docker buildx build --builder "$builder" --platform "$platforms" $tags \
            --output=type=oci,dest=artifacts/revaer-${version}.oci \
            .; \
    else \
        docker buildx build --builder "$builder" --platform "$platforms" $tags \
            --load \
            .; \
    fi

docker-scan:
    if ! command -v trivy >/dev/null 2>&1; then \
        echo "trivy not installed; install it to run this scan" >&2; \
        exit 1; \
    fi
    trivy image --exit-code 1 --severity HIGH,CRITICAL revaer:ci

ui-serve:
    rustup target add wasm32-unknown-unknown
    if ! command -v trunk >/dev/null 2>&1; then \
        cargo install trunk; \
    fi
    cd crates/revaer-ui && trunk serve --open

ui-build:
    rustup target add wasm32-unknown-unknown
    if ! command -v trunk >/dev/null 2>&1; then \
        cargo install trunk; \
    fi
    cd crates/revaer-ui && trunk build --release

dev:
    just db-start
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"; \
    for port in 7070 8080; do \
        pids=$(lsof -ti :$port 2>/dev/null || true); \
        if [ -n "$pids" ]; then \
            echo "Killing processes on port $port: $pids"; \
            kill $pids 2>/dev/null || true; \
        fi; \
    done; \
    if ! command -v cargo-watch >/dev/null 2>&1; then \
        cargo install cargo-watch; \
    fi; \
    rustup target add wasm32-unknown-unknown; \
    if ! command -v trunk >/dev/null 2>&1; then \
        cargo install trunk; \
    fi; \
    DATABASE_URL="${db_url}" RUST_LOG=${RUST_LOG:-debug} cargo watch \
        --ignore 'docs/api/openapi.json' \
        --ignore 'crates/revaer-ui/dist/**' \
        --ignore 'artifacts/**' \
        -x "run -p revaer-app" & \
    api_pid=$!; \
    ( cd crates/revaer-ui && DATABASE_URL="${db_url}" RUST_LOG=${RUST_LOG:-info} trunk serve --open ) & \
    ui_pid=$!; \
    trap 'kill -0 $api_pid 2>/dev/null && kill $api_pid; kill -0 $ui_pid 2>/dev/null && kill $ui_pid' EXIT; \
    wait $api_pid $ui_pid

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

docs-link-check:
    if ! command -v lychee >/dev/null 2>&1; then \
        cargo install --locked lychee; \
    fi
    lychee --verbose --no-progress docs || true

docs:
    just docs-build
    just docs-index

# Start a local Postgres suitable for running the backend and run migrations once the
# container is ready. Uses the dev-friendly defaults unless DATABASE_URL is set.
db-start:
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"; \
    container_name="${PG_CONTAINER:-revaer-db}"; \
    existing_container="$(docker ps -aq -f name=^${container_name}$)"; \
    if [ -n "$existing_container" ]; then \
        if [ -z "$(docker ps -q -f name=^${container_name}$)" ]; then \
            docker start "${container_name}" >/dev/null; \
        fi; \
    else \
        docker run -d \
            --name "${container_name}" \
            -e POSTGRES_USER=revaer \
            -e POSTGRES_PASSWORD=revaer \
            -e POSTGRES_DB=revaer \
            -p 5432:5432 \
            -v revaer-pgdata:/var/lib/postgresql/data \
            postgres:16-alpine >/dev/null; \
    fi; \
    echo "Waiting for Postgres to become ready..."; \
    for _ in $(seq 1 30); do \
        if docker exec "${container_name}" pg_isready -U revaer -d revaer >/dev/null 2>&1; then \
            break; \
        fi; \
        sleep 1; \
    done; \
    if ! command -v sqlx >/dev/null 2>&1; then \
        cargo install sqlx-cli --no-default-features --features postgres; \
    fi; \
    DATABASE_URL="${db_url}" sqlx database create --database-url "${db_url}" 2>/dev/null || true; \
    DATABASE_URL="${db_url}" sqlx migrate run --database-url "${db_url}" --source crates/revaer-data/migrations

# Seed the dev database with a default API key and sensible defaults for local runs.
db-seed:
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"; \
    just db-start; \
    cat scripts/dev-seed.sql | DATABASE_URL="${db_url}" docker exec -i "${PG_CONTAINER:-revaer-db}" psql -U revaer -d revaer >/dev/null
