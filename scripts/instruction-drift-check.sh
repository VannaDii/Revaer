#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${repo_root}"

base_ref="${1:-${REVAER_INSTRUCTION_DIFF_BASE:-}}"
head_ref="${2:-${REVAER_INSTRUCTION_DIFF_HEAD:-HEAD}}"

if [ "${base_ref}" = "0000000000000000000000000000000000000000" ]; then
  base_ref=""
fi

declare -a changed_files=()

load_changed_files() {
  changed_files=()

  while IFS= read -r file; do
    if [ -n "${file}" ]; then
      changed_files+=("${file}")
    fi
  done < <("$@")
}

collect_worktree_changes() {
  load_changed_files bash -lc '
    {
      git diff --name-only --diff-filter=ACMRTUXB
      git diff --name-only --cached --diff-filter=ACMRTUXB
      git ls-files --others --exclude-standard
    } | sort -u
  '
}

if [ -n "${base_ref}" ]; then
  load_changed_files git diff --name-only "${base_ref}" "${head_ref}"
elif [ -n "$(git status --short)" ]; then
  collect_worktree_changes
elif git rev-parse --verify origin/main >/dev/null 2>&1; then
  merge_base="$(git merge-base origin/main HEAD)"
  load_changed_files git diff --name-only "${merge_base}" HEAD
elif git rev-parse --verify HEAD^ >/dev/null 2>&1; then
  load_changed_files git diff --name-only HEAD^ HEAD
else
  exit 0
fi

if [ "${#changed_files[@]}" -eq 0 ]; then
  exit 0
fi

contains_file() {
  local needle="$1"

  for file in "${changed_files[@]}"; do
    if [ "${file}" = "${needle}" ]; then
      return 0
    fi
  done

  return 1
}

collect_matches() {
  local __out_var="$1"
  shift
  local pattern
  local file
  local -a matches=()

  for file in "${changed_files[@]}"; do
    for pattern in "$@"; do
      case "${file}" in
        ${pattern})
          matches+=("${file}")
          break
          ;;
      esac
    done
  done

  if [ "${#matches[@]}" -eq 0 ]; then
    printf -v "${__out_var}" '%s' ""
    return
  fi

  printf -v "${__out_var}" '%s\n' "${matches[@]}"
}

root_updated=false
rust_updated=false
devops_updated=false
sonar_updated=false

if contains_file "AGENTS.md"; then
  root_updated=true
fi
if contains_file ".github/instructions/rust.instructions.md"; then
  rust_updated=true
fi
if contains_file ".github/instructions/devops.instructions.md"; then
  devops_updated=true
fi
if contains_file ".github/instructions/sonarqube_mcp.instructions.md"; then
  sonar_updated=true
fi

collect_matches lint_control_matches \
  "justfile" \
  "scripts/policy-guardrails.sh" \
  "scripts/instruction-drift-check.sh"
collect_matches devops_matches \
  ".github/workflows/*" \
  ".github/actions/*" \
  "Dockerfile" \
  "release/*"
collect_matches sonar_matches \
  ".github/workflows/sonar.yml" \
  "sonar-project.properties"

declare -a failures=()

if [ -n "${lint_control_matches}" ] && ! ${root_updated} && ! ${rust_updated} && ! ${devops_updated}; then
  failures+=(
    "Changed lint/control files require an update to AGENTS.md, .github/instructions/rust.instructions.md, or .github/instructions/devops.instructions.md:
${lint_control_matches}"
  )
fi

if [ -n "${devops_matches}" ] && ! ${root_updated} && ! ${devops_updated}; then
  failures+=(
    "Changed workflow/release files require an update to AGENTS.md or .github/instructions/devops.instructions.md:
${devops_matches}"
  )
fi

if [ -n "${sonar_matches}" ] && ! ${root_updated} && ! ${devops_updated} && ! ${sonar_updated}; then
  failures+=(
    "Changed Sonar files require an update to AGENTS.md, .github/instructions/devops.instructions.md, or .github/instructions/sonarqube_mcp.instructions.md:
${sonar_matches}"
  )
fi

if [ "${#failures[@]}" -eq 0 ]; then
  exit 0
fi

printf 'Instruction drift check failed.\n' >&2
if [ -n "${base_ref}" ]; then
  printf 'Checked diff range: %s..%s\n' "${base_ref}" "${head_ref}" >&2
fi
printf 'Changed files:\n' >&2
printf '  %s\n' "${changed_files[@]}" >&2
printf '\n' >&2
printf '%s\n\n' "${failures[@]}" >&2
exit 1
