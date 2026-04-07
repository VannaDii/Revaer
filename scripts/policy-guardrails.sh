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
rust_files=()

load_rust_files() {
  rust_files=()

  while IFS= read -r file; do
    case "${file}" in
      crates/revaer-ui/ui_vendor/** | \
      crates/revaer-ui/dist/** | \
      crates/revaer-ui/dist-serve/** | \
      crates/revaer-ui/static/nexus/** | \
      tests/node_modules/**)
        continue
        ;;
    esac
    rust_files+=("${file}")
  done < <(
    git ls-files -- \
      ':(glob)crates/**/*.rs' \
      ':(glob)tests/**/*.rs' \
      ':(glob)scripts/**/*.rs'
  )
}

search_rust_matches() {
  local regex="$1"
  local grep_flag="$2"
  shift 2
  local file
  local excluded
  local exclude_pattern
  local -a files=()

  for file in "${rust_files[@]}"; do
    excluded=false
    for exclude_pattern in "$@"; do
      case "${file}" in
        ${exclude_pattern})
          excluded=true
          break
          ;;
      esac
    done
    if ! ${excluded}; then
      files+=("${file}")
    fi
  done

  if [ "${#files[@]}" -eq 0 ]; then
    return 0
  fi

  if command -v rg >/dev/null 2>&1; then
    if [ "${grep_flag}" = "-i" ]; then
      rg -n -i -- "${regex}" "${files[@]}" || true
    else
      rg -n -- "${regex}" "${files[@]}" || true
    fi
  elif [ "${grep_flag}" = "-i" ]; then
    grep -nEi -- "${regex}" "${files[@]}" || true
  else
    grep -nE -- "${regex}" "${files[@]}" || true
  fi
}

load_rust_files

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
  search_rust_matches '#!\[(allow|expect)[[:space:]]*\(|#\[(allow|expect)[[:space:]]*\(' ''
)"
report_matches "source-level lint suppressions are forbidden in authored Rust" "${matches}"

matches="$(
  search_rust_matches 'todo!|unimplemented!' ''
)"
report_matches "todo!/unimplemented! stubs are forbidden in authored Rust" "${matches}"

matches="$(
  search_rust_matches 'sqlx::query(_as|_scalar)?|query!|query_as!|query_scalar!' '' \
    'crates/revaer-data/src/**'
)"
report_matches "sqlx runtime queries are confined to crates/revaer-data/src" "${matches}"

matches="$(
  search_rust_matches '(^|[^[:alpha:]_])(INSERT[[:space:]]+INTO|UPDATE[[:space:]]+("[^"]+"|[[:alpha:]_][[:alnum:]_".]*)[[:space:]]+SET[[:space:]]+[^=[:space:]][^=]*=|DELETE[[:space:]]+FROM|CREATE[[:space:]]+TABLE|ALTER[[:space:]]+TABLE|DROP[[:space:]]+TABLE|TRUNCATE[[:space:]]+TABLE)([^[:alpha:]_]|$)' '-i' \
    'crates/**/src/**/tests.rs'
)"
report_matches "inline DDL/DML is forbidden in Rust; use migrations or stored procedures" "${matches}"

matches="$(
  search_rust_matches '(^|[^[:alnum:]_])catch_unwind([^[:alnum:]_]|$)' '' \
    'crates/revaer-torrent-libt/src/ffi.rs' \
    'crates/revaer-torrent-libt/src/ffi/**'
)"
report_matches "catch_unwind is only allowed at the documented FFI boundary" "${matches}"

matches="$(
  search_rust_matches '(^|[^[:alnum:]_])unsafe([[:space:]]+extern|[[:space:]]+impl|[[:space:]]+fn|[[:space:]]*[{])' '' \
    'crates/revaer-torrent-libt/src/ffi.rs' \
    'crates/revaer-torrent-libt/src/ffi/**'
)"
report_matches "unsafe Rust is only allowed inside the documented FFI boundary" "${matches}"

if [ "${failures}" -ne 0 ]; then
  exit 1
fi
