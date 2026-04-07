#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

files=()
while IFS= read -r file; do
  files+=("${file}")
done < <(find .github/workflows .github/actions -type f \( -name '*.yml' -o -name '*.yaml' \) | sort)

if [ "${#files[@]}" -eq 0 ]; then
  exit 0
fi

failures=0

report_matches() {
  local title="$1"
  local matches="$2"

  if [ -n "${matches}" ]; then
    printf 'Workflow guardrail failed: %s\n' "${title}" >&2
    printf '%s\n' "${matches}" >&2
    printf '\n' >&2
    failures=1
  fi
}

uses_matches="$(
  awk '
    {
      if ($0 !~ /^[-[:space:]]*uses:[[:space:]]*[^[:space:]#]+/) {
        next;
      }

      ref = $0;
      sub(/^[-[:space:]]*uses:[[:space:]]*/, "", ref);
      sub(/[[:space:]]*#.*/, "", ref);

      comment = "";
      if (index($0, "#") > 0) {
        comment = substr($0, index($0, "#"));
      }

      if (ref ~ /^\.\// || ref ~ /^docker:\/\//) {
        next;
      }

      sha = ref;
      sub(/^.*@/, "", sha);

      if (ref !~ /@/ || length(sha) != 40 || sha !~ /^[0-9a-f]+$/) {
        printf "%s:%d:%s\n", FILENAME, FNR, $0;
        next;
      }

      if (comment !~ /#[[:space:]]*v/) {
        printf "%s:%d:%s\n", FILENAME, FNR, $0;
      }
    }
  ' "${files[@]}"
)"
report_matches "external GitHub actions must be pinned by full SHA and carry a version comment" "${uses_matches}"

run_matches="$(
  awk '
    function indent_of(line,    idx, ch) {
      for (idx = 1; idx <= length(line); idx++) {
        ch = substr(line, idx, 1);
        if (ch != " ") {
          return idx - 1;
        }
      }
      return length(line);
    }

    {
      if (in_run) {
        if ($0 !~ /^[[:space:]]*$/ && indent_of($0) <= run_indent) {
          in_run = 0;
        } else {
          if ($0 ~ /\$\{\{[[:space:]]*inputs\./) {
            printf "%s:%d:%s\n", FILENAME, FNR, $0;
          }
          next;
        }
      }

      if ($0 !~ /^[[:space:]]*run:[[:space:]]*/) {
        next;
      }

      run_indent = indent_of($0);
      run_value = $0;
      sub(/^[[:space:]]*run:[[:space:]]*/, "", run_value);

      if (run_value ~ /\$\{\{[[:space:]]*inputs\./) {
        printf "%s:%d:%s\n", FILENAME, FNR, $0;
      }

      if (run_value == "" || run_value ~ /^[|>][-+]?$/) {
        in_run = 1;
      }
    }
  ' "${files[@]}"
)"
report_matches 'workflow run blocks must not interpolate ${{ inputs.* }} directly' "${run_matches}"

if [ "${failures}" -ne 0 ]; then
  exit 1
fi
