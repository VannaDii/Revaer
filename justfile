set shell := ["bash", "-lc"]

fmt:
    cargo fmt --all --check

fmt-fix:
    cargo fmt --all

policy:
    bash scripts/policy-guardrails.sh
    bash scripts/workflow-guardrails.sh

instruction-drift:
    bash scripts/instruction-drift-check.sh

lint: policy
    cargo clippy --workspace --all-targets --all-features -- -D warnings -W clippy::cargo -W clippy::nursery -A clippy::multiple_crate_versions -A clippy::redundant_pub_crate
    cargo clippy --workspace --lib --bins --examples --all-features -- \
        -D warnings \
        -W clippy::expect_used \
        -W clippy::panic \
        -W clippy::todo \
        -W clippy::unimplemented \
        -W clippy::unreachable \
        -W clippy::unwrap_used

check:
    cargo --config 'build.rustflags=["-Dwarnings"]' check --workspace --all-targets --all-features

test:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test --workspace --all-features

test-native:
    REVAER_NATIVE_IT=1 \
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-torrent-libt --all-features

test-features-min:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-api --no-default-features
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}" \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}" \
        cargo --config 'build.rustflags=["-Dwarnings"]' test -p revaer-app --no-default-features

build: sync-assets
    cargo build --workspace --all-targets --all-features

build-release:
    cargo build --workspace --release --all-targets --all-features

release-artifacts: build-release api-export
    mkdir -p dist
    cp target/release/revaer-app dist/revaer-app
    sha256sum dist/revaer-app > dist/revaer-app.sha256
    cp docs/api/openapi.json dist/openapi.json

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
    db_url="${DATABASE_URL:-${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}}"; \
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
    required_llvm_cov_version="0.8.5"; \
    install_llvm_cov() { \
        cargo install cargo-llvm-cov --locked --force --version "${required_llvm_cov_version}"; \
    }; \
    version_ge() { \
        [ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | head -n1)" = "$2" ]; \
    }; \
    if command -v cargo-llvm-cov >/dev/null 2>&1; then \
        installed_version="$(cargo llvm-cov --version | awk '{print $2}')"; \
        if ! version_ge "$installed_version" "$required_llvm_cov_version"; then \
            install_llvm_cov; \
        fi; \
    else \
        install_llvm_cov; \
    fi
    rustup component add llvm-tools-preview
    cargo llvm-cov clean --workspace
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}"; \
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}"; \
        export REVAER_TEST_DATABASE_URL DATABASE_URL; \
    just db-start
    RUST_TEST_THREADS="${RUST_TEST_THREADS:-1}" \
    CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}" \
        cargo llvm-cov --workspace --all-features --no-report
    fail_list=""; \
        while IFS= read -r member; do \
            manifest="${member}/Cargo.toml"; \
            if [ ! -f "${manifest}" ]; then \
                continue; \
            fi; \
            if command -v rg >/dev/null 2>&1; then \
                name="$(rg -m1 '^name = "' "${manifest}" | sed -E 's/^name = "([^"]+)".*/\1/')"; \
            else \
                name="$(grep -m1 '^name = "' "${manifest}" | sed -E 's/^name = "([^"]+)".*/\1/')"; \
            fi; \
            if [ -z "${name}" ]; then \
                continue; \
            fi; \
            echo "== coverage: ${name} =="; \
            if ! cargo llvm-cov report --package "${name}" --json --summary-only --fail-under-lines 90 >/dev/null; then \
                fail_list="${fail_list} ${name}"; \
            fi; \
        done < <(awk ' \
            /^members = \[/ { in_members=1; next } \
            in_members && /^]/ { in_members=0; next } \
            in_members && match($0, /"[^"]+"/) { print substr($0, RSTART + 1, RLENGTH - 2) } \
        ' Cargo.toml); \
        if [ -n "${fail_list}" ]; then \
            echo "Coverage below 90% for:${fail_list}"; \
            exit 1; \
        fi
    rm -rf coverage
    mkdir -p coverage
    cargo llvm-cov report --lcov --output-path coverage/lcov.info
    cargo llvm-cov report --html --output-dir coverage

sonar-compile-db:
    mkdir -p coverage
    rm -f coverage/compile_commands.json
    mkdir -p target/sonar-build
    REVAER_NATIVE_IT=1 \
    CARGO_TARGET_DIR="${PWD}/target/sonar-build" \
    REVAER_NATIVE_COMPILE_COMMANDS_PATH="${PWD}/coverage/compile_commands.json" \
        cargo --config 'build.rustflags=["-Dwarnings"]' build -p revaer-torrent-libt --all-features

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

release-dev:
    npm --prefix release ci
    node release/node_modules/.bin/semantic-release --extends ./release/release.config.js

release-lock:
    npm --prefix release install --package-lock-only

validate:
    REVAER_TEST_DATABASE_URL="${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}"
    DATABASE_URL="${DATABASE_URL:-$REVAER_TEST_DATABASE_URL}"
    export REVAER_TEST_DATABASE_URL DATABASE_URL
    just db-start
    just fmt lint instruction-drift check-assets udeps audit deny ui-build test test-features-min cov

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
    if [ "${CI:-}" = "true" ] || { [ "$(uname -s)" = "Linux" ] && sudo -n true >/dev/null 2>&1; }; then \
        cd tests && npx playwright install --with-deps; \
    else \
        cd tests && npx playwright install; \
    fi
    shard_arg=""; \
    if [ -n "${PLAYWRIGHT_SHARD_INDEX:-}" ] && [ -n "${PLAYWRIGHT_SHARD_TOTAL:-}" ]; then \
        shard_arg="--shard=${PLAYWRIGHT_SHARD_INDEX}/${PLAYWRIGHT_SHARD_TOTAL}"; \
    fi; \
    cd tests && npx playwright test ${shard_arg}

ui-e2e-coverage:
    node tests/scripts/check-e2e-coverage.js

runbook:
    status=0; \
    just ui-e2e || status=$?; \
    if [ "$status" -eq 0 ]; then \
        just ui-e2e-coverage || status=$?; \
    fi; \
    mkdir -p artifacts/runbook; \
    rm -rf artifacts/runbook/logs artifacts/runbook/playwright-report artifacts/runbook/test-results; \
    if [ -d tests/logs ]; then \
        cp -R tests/logs artifacts/runbook/logs; \
    fi; \
    if [ -d tests/playwright-report ]; then \
        cp -R tests/playwright-report artifacts/runbook/playwright-report; \
    fi; \
    if [ -d tests/test-results ]; then \
        cp -R tests/test-results artifacts/runbook/test-results; \
        rm -f artifacts/runbook/test-results/e2e-state.json; \
    fi; \
    runbook_status="ok"; \
    if [ "$status" -ne 0 ]; then \
        runbook_status="failed"; \
    fi; \
    printf '%s\n' \
        "runbook=${runbook_status}" \
        "artifacts=artifacts/runbook" \
        "playwright_report=artifacts/runbook/playwright-report/index.html" \
        "test_results=artifacts/runbook/test-results" \
        "logs=artifacts/runbook/logs" \
        > artifacts/runbook/summary.txt; \
    exit "$status"

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
    check_port_free() { \
        port="$1"; \
        name="$2"; \
        if [ "${DEV_SKIP_PORT_CHECK:-0}" = "1" ]; then \
            return 0; \
        fi; \
        if (echo >/dev/tcp/127.0.0.1/"${port}") >/dev/null 2>&1; then \
            echo "${name} port ${port} is already in use. Stop the existing service or set DEV_SKIP_PORT_CHECK=1 to skip." >&2; \
            return 1; \
        fi; \
    }; \
    check_port_free 7070 "API"; \
    check_port_free 8080 "UI"; \
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
    terminate_pid() { \
        pid="$1"; \
        if [ -z "$pid" ]; then \
            return 0; \
        fi; \
        if ! kill -0 "$pid" 2>/dev/null; then \
            return 0; \
        fi; \
        kill "$pid" 2>/dev/null || true; \
        for _ in $(seq 1 25); do \
            if ! kill -0 "$pid" 2>/dev/null; then \
                return 0; \
            fi; \
            sleep 0.2; \
        done; \
        kill -9 "$pid" 2>/dev/null || true; \
    }; \
    kill_tree() { \
        pid="$1"; \
        if [ -z "$pid" ]; then \
            return 0; \
        fi; \
        children="$(ps -o pid= --ppid "$pid" 2>/dev/null || true)"; \
        for child in $children; do \
            kill_tree "$child"; \
        done; \
        terminate_pid "$pid"; \
    }; \
    cleanup_dev() { \
        trap - EXIT INT TERM; \
        kill_tree "$api_pid"; \
        kill_tree "$ui_pid"; \
    }; \
    trap cleanup_dev EXIT INT TERM; \
    wait_for_port() { \
        port="$1"; \
        name="$2"; \
        pid="$3"; \
        timeout="${DEV_STARTUP_TIMEOUT:-180}"; \
        deadline=$((SECONDS + timeout)); \
        while [ "$SECONDS" -lt "$deadline" ]; do \
            if ! kill -0 "$pid" 2>/dev/null; then \
                echo "${name} exited before it opened port ${port}" >&2; \
                return 1; \
            fi; \
            if (echo >/dev/tcp/127.0.0.1/"${port}") >/dev/null 2>&1; then \
                return 0; \
            fi; \
            sleep 0.2; \
        done; \
        echo "Timed out waiting for ${name} to listen on port ${port}" >&2; \
        return 1; \
    }; \
    echo "Waiting for API (7070) and UI (8080) to start..."; \
    wait_for_port 7070 "API" "$api_pid"; \
    wait_for_port 8080 "Trunk" "$ui_pid"; \
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
    db_host="$(printf "%s" "${db_url}" | sed -E 's#^[^:]+://[^@]+@([^:/]+).*#\1#')"; \
    db_port="$(printf "%s" "${db_url}" | sed -En 's#^[^:]+://[^@]+@[^:/]+:([0-9]+).*#\1#p')"; \
    if [ -z "${db_port}" ]; then \
        db_port="5432"; \
    fi; \
    if [ "${db_host}" = "host.docker.internal" ] && python3 -c 'import socket, sys; probe = socket.create_connection((sys.argv[1], int(sys.argv[2])), 1); probe.close()' localhost "${db_port}" >/dev/null 2>&1; then \
        db_host="localhost"; \
        db_url="$(printf "%s" "${db_url}" | sed 's#@host\.docker\.internal\([:/]\)#@localhost\1#')"; \
        echo "Normalized local Docker database host to ${db_host}:${db_port}"; \
    fi; \
    echo "Using database URL: ${db_url}"; \
    container_name="${PG_CONTAINER:-revaer-db}"; \
    db_data_dir="${PWD}/.server_root/postgres-data"; \
    mkdir -p "${db_data_dir}"; \
    existing_container="$(docker ps -aq -f name=^${container_name}$)"; \
    if [ -n "${existing_container}" ] && [ -z "$(docker ps -q -f name=^${container_name}$)" ]; then \
        if docker logs --tail 50 "${container_name}" 2>&1 | grep -q 'No space left on device'; then \
            echo "Recreating failed Postgres container (${container_name}) with host-backed storage"; \
            docker rm -f "${container_name}" >/dev/null 2>&1 || true; \
            existing_container=""; \
        fi; \
    fi; \
    if python3 -c 'import socket, sys; probe = socket.create_connection((sys.argv[1], int(sys.argv[2])), 1); probe.close()' "${db_host}" "${db_port}" >/dev/null 2>&1; then \
        echo "Using existing Postgres endpoint ${db_host}:${db_port}"; \
    else \
        if [ -n "$existing_container" ]; then \
            published_port="$(docker port "${container_name}" 5432/tcp 2>/dev/null || true)"; \
            if [ -z "$published_port" ]; then \
                echo "Recreating existing Postgres container (${container_name}) without a published host port"; \
                docker rm -f "${container_name}" >/dev/null; \
                existing_container=""; \
            elif ! printf "%s" "$published_port" | grep -Eq "(:|^)${db_port}$"; then \
                echo "Recreating existing Postgres container (${container_name}) with mismatched published port ${published_port}"; \
                docker rm -f "${container_name}" >/dev/null; \
                existing_container=""; \
            fi; \
        fi; \
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
                -p "${db_port}:5432" \
                -v "${db_data_dir}:/var/lib/postgresql/data" \
                postgres:16-alpine >/dev/null; \
        fi; \
        echo "Waiting for Postgres to become ready..."; \
        for _ in $(seq 1 30); do \
            if docker exec "${container_name}" pg_isready -U revaer -d postgres >/dev/null 2>&1; then \
                break; \
            fi; \
            sleep 1; \
        done; \
    fi; \
    echo "Waiting for external Postgres endpoint ${db_host}:${db_port}..."; \
    external_ready="0"; \
    for _ in $(seq 1 30); do \
        if python3 -c 'import socket, sys; probe = socket.create_connection((sys.argv[1], int(sys.argv[2])), 1); probe.close()' "${db_host}" "${db_port}" >/dev/null 2>&1; then \
            external_ready="1"; \
            break; \
        fi; \
        sleep 1; \
    done; \
    if [ "${external_ready}" != "1" ]; then \
        echo "Postgres endpoint ${db_host}:${db_port} did not become reachable."; \
        exit 1; \
    fi; \
    wait_for_local_postgres_writable() { \
        attempt="1"; \
        while [ "${attempt}" -le 60 ]; do \
            writable="$(docker exec -e PGPASSWORD=revaer "${container_name}" psql -U revaer -d postgres -Atqc 'SELECT CASE WHEN pg_is_in_recovery() THEN 0 ELSE 1 END' 2>/dev/null | tr -d '[:space:]')"; \
            if [ "${writable}" = "1" ]; then \
                return 0; \
            fi; \
            echo "Local Postgres is still in recovery; waiting for writable state (attempt ${attempt}/60)..."; \
            attempt="$((attempt + 1))"; \
            sleep 1; \
        done; \
        echo "Local Postgres container ${container_name} did not exit recovery in time."; \
        return 1; \
    }; \
    if [ -n "${existing_container}" ]; then \
        wait_for_local_postgres_writable; \
    fi; \
    just sqlx-install; \
    run_sqlx_with_recovery_retry() { \
        attempt="1"; \
        while true; do \
            output="$("$@" 2>&1)"; \
            status=$?; \
            if [ "${status}" -eq 0 ]; then \
                if [ -n "${output}" ]; then \
                    printf '%s\n' "${output}"; \
                fi; \
                return 0; \
            fi; \
            if printf '%s' "${output}" | grep -Eq 'the database system is (in recovery mode|starting up|not yet accepting connections)|consistent recovery state has not been yet reached'; then \
                printf '%s\n' "${output}" >&2; \
                if [ "${attempt}" -ge 30 ]; then \
                    echo "Postgres did not become writable in time." >&2; \
                    return 2; \
                fi; \
                echo "Postgres is still recovering; retrying in 1s (attempt ${attempt}/30)..."; \
                attempt="$((attempt + 1))"; \
                sleep 1; \
                continue; \
            fi; \
            printf '%s\n' "${output}" >&2; \
            return "${status}"; \
        done; \
    }; \
    DATABASE_URL="${db_url}" sqlx database create --database-url "${db_url}" 2>/dev/null || true; \
    reset_db="${REVAER_DB_RESET:-0}"; \
    if [ "${reset_db}" = "1" ]; then \
        if echo "${db_url}" | grep -Eq '@(localhost|127\.0\.0\.1|host\.docker\.internal)(:|/)'; then \
            echo "Resetting local database..."; \
            if run_sqlx_with_recovery_retry env DATABASE_URL="${db_url}" sqlx database reset -y --database-url "${db_url}" --source crates/revaer-data/migrations; then \
                reset_status="0"; \
            else \
                reset_status="$?"; \
            fi; \
            if [ "${reset_status}" -ne 0 ]; then \
                if [ "${reset_status}" -eq 2 ]; then \
                    exit 1; \
                fi; \
                exit "${reset_status}"; \
            fi; \
        else \
            echo "Reset requested for ${db_url}; refusing to reset non-local database."; \
            exit 1; \
        fi; \
    else \
        if run_sqlx_with_recovery_retry env DATABASE_URL="${db_url}" sqlx migrate run --database-url "${db_url}" --source crates/revaer-data/migrations; then \
            migrate_status="0"; \
        else \
            migrate_status="$?"; \
        fi; \
        if [ "${migrate_status}" -ne 0 ]; then \
            if [ "${migrate_status}" -eq 2 ]; then \
                exit 1; \
            fi; \
            if echo "${db_url}" | grep -Eq '@(localhost|127\.0\.0\.1|host\.docker\.internal)(:|/)'; then \
                echo "Migration history mismatch; resetting local database..."; \
                if run_sqlx_with_recovery_retry env DATABASE_URL="${db_url}" sqlx database reset -y --database-url "${db_url}" --source crates/revaer-data/migrations; then \
                    reset_status="0"; \
                else \
                    reset_status="$?"; \
                fi; \
                if [ "${reset_status}" -ne 0 ]; then \
                    if [ "${reset_status}" -eq 2 ]; then \
                        exit 1; \
                    fi; \
                    exit "${reset_status}"; \
                fi; \
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
    db_url="${DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}"; \
    just db-start; \
    cat scripts/dev-seed.sql | DATABASE_URL="${db_url}" docker exec -i "${PG_CONTAINER:-revaer-db}" psql -U revaer -d revaer >/dev/null
