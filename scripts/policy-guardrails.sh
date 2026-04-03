#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

readonly rust_paths=(
  crates
  tests
  scripts
)

readonly exclude_globs=(
  --glob '!crates/revaer-ui/ui_vendor/**'
  --glob '!crates/revaer-ui/dist/**'
  --glob '!crates/revaer-ui/dist-serve/**'
  --glob '!crates/revaer-ui/static/nexus/**'
  --glob '!tests/node_modules/**'
)

failures=0

report_matches() {
  local title="$1"
  local matches="$2"

  if [ -n "${matches}" ]; then
    printf 'Policy guardrail failed: %s\n' "${title}" >&2
    printf '%s\n' "${matches}" >&2
    printf '\n' >&2
    failures=1
  fi
}

matches="$(
  rg -n --glob '*.rs' '#!\[(allow|expect)\(|#\[(allow|expect)\(' \
    "${rust_paths[@]}" \
    "${exclude_globs[@]}" \
    || true
)"
report_matches "source-level lint suppressions are forbidden in authored Rust" "${matches}"

matches="$(
  rg -n --glob '*.rs' 'todo!|unimplemented!' \
    "${rust_paths[@]}" \
    "${exclude_globs[@]}" \
    || true
)"
report_matches "todo!/unimplemented! stubs are forbidden in authored Rust" "${matches}"

matches="$(
  rg -n --glob '*.rs' '\bcatch_unwind\b' \
    "${rust_paths[@]}" \
    "${exclude_globs[@]}" \
    --glob '!crates/revaer-torrent-libt/src/ffi.rs' \
    --glob '!crates/revaer-torrent-libt/src/ffi/**' \
    || true
)"
report_matches "catch_unwind is only allowed at the documented FFI boundary" "${matches}"

matches="$(
  rg -n --glob '*.rs' '(^|[^[:alnum:]_])unsafe([[:space:]]+extern|[[:space:]]+impl|[[:space:]]+fn|[[:space:]]*[{])' \
    "${rust_paths[@]}" \
    "${exclude_globs[@]}" \
    --glob '!crates/revaer-torrent-libt/src/ffi.rs' \
    --glob '!crates/revaer-torrent-libt/src/ffi/**' \
    || true
)"
report_matches "unsafe Rust is only allowed inside the documented FFI boundary" "${matches}"

if [ "${failures}" -ne 0 ]; then
  exit 1
fi
