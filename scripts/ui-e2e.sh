#!/usr/bin/env bash
set -euo pipefail

root_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
tests_dir="${root_dir}/tests"
env_file="${tests_dir}/.env"

if [ -f "${env_file}" ]; then
  set -a
  # shellcheck disable=SC1090
  source "${env_file}"
  set +a
fi

E2E_API_BASE_URL="${E2E_API_BASE_URL:-http://localhost:7070}"
E2E_BASE_URL="${E2E_BASE_URL:-http://localhost:8080}"
E2E_DB_ADMIN_URL="${E2E_DB_ADMIN_URL:-${REVAER_TEST_DATABASE_URL:-postgres://revaer:revaer@localhost:5432/postgres}}"
E2E_DB_PREFIX="${E2E_DB_PREFIX:-revaer_e2e}"
E2E_FS_ROOT="${E2E_FS_ROOT:-${root_dir}}"
E2E_SESSION_PATH="${E2E_SESSION_PATH:-.auth/session.json}"

export E2E_API_BASE_URL E2E_BASE_URL E2E_DB_ADMIN_URL E2E_DB_PREFIX E2E_FS_ROOT E2E_SESSION_PATH

cd "${root_dir}"

db_url_reachable() {
  python3 - "$1" >/dev/null 2>&1 <<'PY'
import socket
import urllib.parse
import sys

u = urllib.parse.urlparse(sys.argv[1])
host = u.hostname
port = u.port or 5432
try:
    with socket.create_connection((host, port), timeout=1):
        pass
    sys.exit(0)
except Exception:
    sys.exit(1)
PY
}

if ! db_url_reachable "${E2E_DB_ADMIN_URL}"; then
  if [ -n "${REVAER_TEST_DATABASE_URL:-}" ] && db_url_reachable "${REVAER_TEST_DATABASE_URL}"; then
    E2E_DB_ADMIN_URL="${REVAER_TEST_DATABASE_URL}"
  elif [ -n "${DATABASE_URL:-}" ] && db_url_reachable "${DATABASE_URL}"; then
    E2E_DB_ADMIN_URL="${DATABASE_URL}"
  fi
  export E2E_DB_ADMIN_URL
fi

host="$(python3 -c "import urllib.parse,sys; print(urllib.parse.urlparse(sys.argv[1]).hostname or '')" "${E2E_DB_ADMIN_URL}")"
if printf '%s' "${host}" | grep -Eq '^(localhost|127\.0\.0\.1|host\.docker\.internal)$'; then
  db_start_url="$(python3 -c "import urllib.parse,sys; base=urllib.parse.urlparse(sys.argv[1]); path='/revaer'; print(urllib.parse.urlunparse(base._replace(path=path)))" "${E2E_DB_ADMIN_URL}")"
  DATABASE_URL="${db_start_url}" just db-start
fi
just sqlx-install

cd "${tests_dir}"
npm install
npx playwright install
cd "${root_dir}"

cargo build -p revaer-app
API_BIN="${root_dir}/target/debug/revaer-app"
if [ ! -x "${API_BIN}" ]; then
  echo "revaer-app binary not found at ${API_BIN}" >&2
  exit 1
fi

log_dir="${tests_dir}/logs"
mkdir -p "${log_dir}"

API_PID=""
UI_PID=""
ACTIVE_DB_URL=""

stop_dev_servers() {
  local patterns=(
    "cargo run -p revaer-app"
    "cargo run -p revaer-ui"
    "trunk serve"
    "target/debug/revaer-app"
    "target/release/revaer-app"
  )
  for pattern in "${patterns[@]}"; do
    while read -r pid; do
      if [ -z "${pid}" ] || [ "${pid}" = "$$" ]; then
        continue
      fi
      local cmd
      cmd="$(ps -p "${pid}" -o args= 2>/dev/null || true)"
      case "${cmd}" in
        *"scripts/ui-e2e.sh"*|*"pgrep -f"*|*"ps -p"*)
          continue
          ;;
      esac
      if [ -n "${cmd}" ]; then
        echo "Stopping existing Revaer dev process (pid ${pid}: ${cmd})" >&2
      fi
      kill "${pid}" >/dev/null 2>&1 || true
    done < <(pgrep -f "${pattern}" 2>/dev/null || true)
  done
}

port_in_use() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    lsof -ti :"${port}" >/dev/null 2>&1
    return $?
  fi
  python3 - "${port}" >/dev/null 2>&1 <<'PY'
import socket
import sys

port = int(sys.argv[1])
try:
    with socket.create_connection(("127.0.0.1", port), timeout=0.2):
        pass
    sys.exit(0)
except Exception:
    sys.exit(1)
PY
}

pids_on_port() {
  local port="$1"
  if ! command -v lsof >/dev/null 2>&1; then
    return 0
  fi
  lsof -ti :"${port}" 2>/dev/null || true
}

stop_known_dev_processes() {
  local port="$1"
  local pids
  pids="$(pids_on_port "${port}")"
  if [ -z "${pids}" ]; then
    return 0
  fi

  local stopped=false
  for pid in ${pids}; do
    local cmd
    cmd="$(ps -p "${pid}" -o args= 2>/dev/null || true)"
    if [ -z "${cmd}" ]; then
      continue
    fi
    if printf '%s' "${cmd}" | grep -Eq 'revaer-app|revaer-ui|trunk serve|cargo run -p revaer-app|cargo run -p revaer-ui'; then
      echo "Stopping existing Revaer dev process on port ${port} (pid ${pid}: ${cmd})" >&2
      kill "${pid}" >/dev/null 2>&1 || true
      stopped=true
    else
      echo "Port ${port} is in use by a non-Revaer process: ${cmd}" >&2
      return 1
    fi
  done

  if [ "${stopped}" = true ]; then
    for _ in $(seq 1 20); do
      if ! port_in_use "${port}"; then
        return 0
      fi
      sleep 0.25
    done
  fi

  return 1
}

require_port_free() {
  local port="$1"
  if port_in_use "${port}"; then
    if stop_known_dev_processes "${port}"; then
      return 0
    fi
    echo "Port ${port} is in use; stop existing services before running ui-e2e." >&2
    exit 1
  fi
}

http_ready() {
  local url="$1"
  if command -v curl >/dev/null 2>&1; then
    curl -sf "${url}" >/dev/null 2>&1
    return $?
  fi
  python3 - "${url}" >/dev/null 2>&1 <<'PY'
import sys
import urllib.request

try:
    with urllib.request.urlopen(sys.argv[1], timeout=1):
        pass
    sys.exit(0)
except Exception:
    sys.exit(1)
PY
}

wait_for_http() {
  local url="$1"
  local attempts="${2:-60}"
  for _ in $(seq 1 "${attempts}"); do
    if http_ready "${url}"; then
      return 0
    fi
    sleep 0.5
  done
  echo "Timed out waiting for ${url}" >&2
  return 1
}

build_db_url() {
  local base="$1"
  local name="$2"
  python3 -c "import urllib.parse,sys; base=urllib.parse.urlparse(sys.argv[1]); name=sys.argv[2]; \
path='/' + name; \
print(urllib.parse.urlunparse(base._replace(path=path)))" "${base}" "${name}"
}

create_temp_db() {
  local run_id
  run_id="$(date +%s)_${RANDOM}"
  local db_name="${E2E_DB_PREFIX}_${run_id}"
  local db_url
  db_url="$(build_db_url "${E2E_DB_ADMIN_URL}" "${db_name}")"
  DATABASE_URL="${db_url}" sqlx database create --database-url "${db_url}" >/dev/null
  DATABASE_URL="${db_url}" sqlx migrate run --database-url "${db_url}" --source crates/revaer-data/migrations >/dev/null
  echo "${db_url}"
}

drop_temp_db() {
  local db_url="$1"
  local db_host
  db_host="$(python3 -c "import urllib.parse,sys; print(urllib.parse.urlparse(sys.argv[1]).hostname or '')" "${db_url}")"
  case "${db_host}" in
    localhost|127.0.0.1|host.docker.internal) ;;
    *)
      echo "Refusing to drop non-local database (${db_host})." >&2
      return 1
      ;;
  esac
  DATABASE_URL="${db_url}" sqlx database drop --database-url "${db_url}" -y
}

assert_api_db() {
  local pid="$1"
  local expected="$2"
  if ! kill -0 "${pid}" >/dev/null 2>&1; then
    echo "API process exited before it became ready." >&2
    return 1
  fi
  if [ -r "/proc/${pid}/environ" ]; then
    local actual
    actual="$(tr '\0' '\n' < "/proc/${pid}/environ" | grep -E '^DATABASE_URL=' | head -n 1)"
    actual="${actual#DATABASE_URL=}"
    if [ -n "${actual}" ] && [ "${actual}" != "${expected}" ]; then
      echo "API process started with unexpected DATABASE_URL: ${actual}" >&2
      return 1
    fi
  fi
  return 0
}

assert_api_listener() {
  local pid="$1"
  local port="$2"
  if command -v lsof >/dev/null 2>&1; then
    local listener
    listener="$(lsof -tiTCP:"${port}" -sTCP:LISTEN 2>/dev/null | head -n 1 || true)"
    if [ -n "${listener}" ] && [ "${listener}" != "${pid}" ]; then
      echo "Port ${port} is already bound by pid ${listener}; expected ${pid}." >&2
      return 1
    fi
  fi
  return 0
}

cleanup_resources() {
  if [ -n "${UI_PID}" ] && kill -0 "${UI_PID}" >/dev/null 2>&1; then
    kill "${UI_PID}" >/dev/null 2>&1 || true
    wait "${UI_PID}" >/dev/null 2>&1 || true
  fi
  if [ -n "${API_PID}" ] && kill -0 "${API_PID}" >/dev/null 2>&1; then
    kill "${API_PID}" >/dev/null 2>&1 || true
    wait "${API_PID}" >/dev/null 2>&1 || true
  fi
  if [ -n "${ACTIVE_DB_URL}" ]; then
    drop_temp_db "${ACTIVE_DB_URL}" >/dev/null 2>&1 || true
  fi
}

trap cleanup_resources EXIT

run_api_suite() {
  local auth_mode="$1"
  require_port_free 7070
  ACTIVE_DB_URL="$(create_temp_db)"
  DATABASE_URL="${ACTIVE_DB_URL}" "${API_BIN}" > "${log_dir}/api-${auth_mode}.log" 2>&1 &
  API_PID="$!"
  assert_api_db "${API_PID}" "${ACTIVE_DB_URL}"
  wait_for_http "${E2E_API_BASE_URL}/health" 80
  assert_api_listener "${API_PID}" 7070
  (cd "${tests_dir}" && E2E_AUTH_MODE="${auth_mode}" npx playwright test --project api)
  cleanup_resources
  API_PID=""
  ACTIVE_DB_URL=""
}

run_ui_suite() {
  local auth_mode="$1"
  require_port_free 7070
  require_port_free 8080
  ACTIVE_DB_URL="$(create_temp_db)"
  DATABASE_URL="${ACTIVE_DB_URL}" "${API_BIN}" > "${log_dir}/api-ui.log" 2>&1 &
  API_PID="$!"
  assert_api_db "${API_PID}" "${ACTIVE_DB_URL}"
  wait_for_http "${E2E_API_BASE_URL}/health" 80
  assert_api_listener "${API_PID}" 7070
  just sync-assets
  rustup target add wasm32-unknown-unknown
  if ! command -v trunk >/dev/null 2>&1; then
    cargo install trunk
  fi
  mkdir -p "${root_dir}/crates/revaer-ui/dist-serve/.stage"
  (cd "${root_dir}/crates/revaer-ui" && DATABASE_URL="${ACTIVE_DB_URL}" RUST_LOG=${RUST_LOG:-info} trunk serve --dist dist-serve --port 8080) > "${log_dir}/ui.log" 2>&1 &
  UI_PID="$!"
  wait_for_http "${E2E_BASE_URL}" 80
  ui_projects=()
  IFS=',' read -ra browsers <<< "${E2E_BROWSERS:-chromium}"
  for browser in "${browsers[@]}"; do
    name="$(echo "${browser}" | xargs)"
    if [ -n "${name}" ]; then
      ui_projects+=(--project "ui-${name}")
    fi
  done
  if [ "${#ui_projects[@]}" -eq 0 ]; then
    ui_projects=(--project ui-chromium)
  fi
  (cd "${tests_dir}" && E2E_AUTH_MODE="${auth_mode}" npx playwright test "${ui_projects[@]}")
  cleanup_resources
  UI_PID=""
  API_PID=""
  ACTIVE_DB_URL=""
}

stop_dev_servers
run_api_suite api_key
run_api_suite none
run_ui_suite api_key
