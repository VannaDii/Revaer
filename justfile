set shell := ["bash", "-lc"]

fmt:
    cargo fmt --all --check

fmt-fix:
    cargo fmt --all

lint:
    cargo clippy --workspace --all-targets --all-features -- -D warnings -A clippy::multiple_crate_versions

check:
    cargo --config 'build.rustflags=["-Dwarnings"]' check --workspace --all-targets --all-features

test:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test --workspace --all-features

test-native:
    REVAER_NATIVE_IT=1 \
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-torrent-libt --all-features

test-features-min:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-api --no-default-features
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-app --no-default-features

build: sync-assets
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

sqlx-install:
    if ! command -v sqlx >/dev/null 2>&1; then \
        cargo install sqlx-cli --no-default-features --features postgres; \
    fi

db-migrate: sqlx-install
    db_url="${DATABASE_URL:-${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}}"; \
    DATABASE_URL="${db_url}" sqlx migrate run --source crates/revaer-data/migrations

audit:
    required_audit_version="0.22.0"; \
    install_audit() { \
        cargo install cargo-audit --locked --force --version "${required_audit_version}"; \
    }; \
    version_ge() { \
        [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$2" ]; \
    }; \
    if command -v cargo-audit >/dev/null 2>&1; then \
        installed_version="$(cargo audit -V | awk 'NR==1 {print $2}')"; \
        if ! version_ge "$installed_version" "$required_audit_version"; then \
            install_audit; \
        fi; \
    else \
        install_audit; \
    fi; \
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
    required_deny_version="0.18.9"; \
    install_deny() { \
        cargo install cargo-deny --locked --force --version "${required_deny_version}"; \
    }; \
    version_ge() { \
        [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$2" ]; \
    }; \
    if command -v cargo-deny >/dev/null 2>&1; then \
        installed_version="$(cargo deny --version | awk 'NR==1 {print $2}')"; \
        if ! version_ge "$installed_version" "$required_deny_version"; then \
            install_deny; \
        fi; \
    else \
        install_deny; \
    fi
    cargo deny check

cov:
    if ! command -v cargo-llvm-cov >/dev/null 2>&1; then \
        cargo install cargo-llvm-cov --locked; \
    fi
    rustup component add llvm-tools-preview
    cargo llvm-cov clean --workspace
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        fail_list=""; \
        while IFS= read -r member; do \
            manifest="${member}/Cargo.toml"; \
            if [ ! -f "${manifest}" ]; then \
                continue; \
            fi; \
            if command -v rg >/dev/null 2>&1; then \
                name="$(rg -m1 '^name = \"' "${manifest}" | sed -E 's/^name = \"([^\"]+)\".*/\\1/')"; \
            else \
                name="$(grep -m1 '^name = \"' "${manifest}" | sed -E 's/^name = \"([^\"]+)\".*/\\1/')"; \
            fi; \
            if [ -z "${name}" ]; then \
                continue; \
            fi; \
            echo "== coverage: ${name} =="; \
            if ! cargo llvm-cov --package "${name}" --fail-under-lines 90; then \
                fail_list="${fail_list} ${name}"; \
            fi; \
        done < <(awk '/^members = \\[/{in_members=1;next} in_members && /^]/{in_members=0} in_members { if (match($0, /\"[^\"]+\"/)) print substr($0, RSTART + 1, RLENGTH - 2) }' Cargo.toml); \
        if [ -n "${fail_list}" ]; then \
            echo "Coverage below 90% for:${fail_list}"; \
            exit 1; \
        fi

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

validate:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}"
    export REVAER_TEST_DATABASE_URL DATABASE_URL
    just db-start
    just fmt lint check-assets udeps audit deny ui-build test test-features-min cov

ci: validate
    just build-release

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

sync-assets:
    cargo run -p asset_sync

check-assets: sync-assets
    git diff --exit-code -- static/nexus

ui-serve: sync-assets
    rustup target add wasm32-unknown-unknown
    if ! command -v trunk >/dev/null 2>&1; then \
        cargo install trunk; \
    fi
    mkdir -p crates/revaer-ui/dist-serve/.stage
    cd crates/revaer-ui && NO_COLOR=true trunk serve --dist dist-serve --open

ui-build: sync-assets
    rustup target add wasm32-unknown-unknown
    if ! command -v trunk >/dev/null 2>&1; then \
        cargo install trunk; \
    fi
    mkdir -p crates/revaer-ui/dist/.stage
    cd crates/revaer-ui && NO_COLOR=true trunk build --release

ui-e2e:
    cd tests && npm install
    cd tests && npm run gen:api-client
    cd tests && npx playwright install
    cd tests && npx playwright test

zombies:
    for port in 7070 8080; do \
        pids=$(lsof -ti :$port 2>/dev/null || true); \
        if [ -z "$pids" ]; then \
            continue; \
        fi; \
        echo "Stopping processes on port $port: $pids"; \
        kill $pids 2>/dev/null || true; \
        for pid in $pids; do \
            for _ in 1 2 3 4 5 6 7 8 9 10; do \
                if ! kill -0 "$pid" 2>/dev/null; then \
                    break; \
                fi; \
                sleep 0.2; \
            done; \
            if kill -0 "$pid" 2>/dev/null; then \
                echo "Force killing process $pid on port $port"; \
                kill -9 "$pid" 2>/dev/null || true; \
            fi; \
        done; \
        remaining=$(lsof -ti :$port 2>/dev/null || true); \
        if [ -n "$remaining" ]; then \
            echo "Processes still bound to port $port: $remaining" >&2; \
            exit 1; \
        fi; \
    done

dev: sync-assets
    just db-start
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"; \
    just zombies; \
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
        --ignore 'crates/revaer-ui/dist-serve/**' \
        --ignore 'artifacts/**' \
        -x "run -p revaer-app" & \
    api_pid=$!; \
    mkdir -p crates/revaer-ui/dist-serve/.stage; \
    ( cd crates/revaer-ui && DATABASE_URL="${db_url}" RUST_LOG=${RUST_LOG:-info} NO_COLOR=true trunk serve --dist dist-serve ) & \
    ui_pid=$!; \
    trap 'kill -0 $api_pid 2>/dev/null && kill $api_pid; kill -0 $ui_pid 2>/dev/null && kill $ui_pid' EXIT; \
    wait $api_pid $ui_pid

docs-install:
    required_mdbook_mermaid_version="0.17.0"; \
    if ! command -v mdbook >/dev/null 2>&1; then \
        cargo install --locked mdbook; \
    fi; \
    if ! command -v mdbook-mermaid >/dev/null 2>&1; then \
        cargo install --locked mdbook-mermaid --version "$required_mdbook_mermaid_version"; \
    else \
        current_mdbook_mermaid_version="$(mdbook-mermaid --version | awk '{print $2}')"; \
        if [ "$current_mdbook_mermaid_version" != "$required_mdbook_mermaid_version" ]; then \
            cargo install --locked mdbook-mermaid --version "$required_mdbook_mermaid_version" --force; \
        fi; \
    fi; \
    mdbook-mermaid install ./docs

docs-build:
    cd docs && mdbook build

docs-serve:
    cd docs && mdbook serve --open

docs-index:
    cargo run -p revaer-doc-indexer --release

docs-link-check:
    if ! command -v lychee >/dev/null 2>&1; then \
        cargo install --locked lychee; \
    fi
    lychee --verbose --no-progress docs || true

docs:
    just docs-install
    just docs-build
    just docs-index

# Start a local Postgres suitable for running the backend and run migrations once the
# container is ready. Uses the dev-friendly defaults unless DATABASE_URL is set.
# Set REVAER_DB_RESET=1 to drop + recreate local databases before running migrations.
db-start:
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"; \
    echo "Using database URL: ${db_url}"; \
    container_name="${PG_CONTAINER:-revaer-db}"; \
    existing_container="$(docker ps -aq -f name=^${container_name}$)"; \
    if [ -n "$existing_container" ]; then \
        if [ -z "$(docker ps -q -f name=^${container_name}$)" ]; then \
            echo "Starting existing Postgres container (${container_name})"; \
            docker start "${container_name}" >/dev/null; \
        fi; \
    else \
        echo "Starting new Postgres container (${container_name})"; \
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
    just sqlx-install; \
    DATABASE_URL="${db_url}" sqlx database create --database-url "${db_url}" 2>/dev/null || true; \
    reset_db="${REVAER_DB_RESET:-0}"; \
    if [ "${reset_db}" = "1" ]; then \
        if echo "${db_url}" | grep -Eq '@(localhost|127\\.0\\.0\\.1)(:|/)'; then \
            echo "Resetting local database..."; \
            DATABASE_URL="${db_url}" sqlx database reset -y --database-url "${db_url}" --source crates/revaer-data/migrations; \
        else \
            echo "Reset requested for ${db_url}; refusing to reset non-local database."; \
            exit 1; \
        fi; \
    else \
        if ! DATABASE_URL="${db_url}" sqlx migrate run --database-url "${db_url}" --source crates/revaer-data/migrations; then \
            if echo "${db_url}" | grep -Eq '@(localhost|127\\.0\\.0\\.1)(:|/)'; then \
                echo "Migration history mismatch; resetting local database..."; \
                DATABASE_URL="${db_url}" sqlx database reset -y --database-url "${db_url}" --source crates/revaer-data/migrations; \
            else \
                echo "Migration history mismatch for ${db_url}; refusing to reset non-local database."; \
                exit 1; \
            fi; \
        fi; \
    fi

db-reset:
    REVAER_DB_RESET=1 just db-start

# Seed the dev database with a default API key and sensible defaults for local runs.
db-seed:
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/revaer}"; \
    just db-start; \
    cat scripts/dev-seed.sql | DATABASE_URL="${db_url}" docker exec -i "${PG_CONTAINER:-revaer-db}" psql -U revaer -d revaer >/dev/null
