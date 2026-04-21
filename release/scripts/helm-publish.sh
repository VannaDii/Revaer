#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 2 ]]; then
    echo "usage: $0 <chart-version> <app-version>" >&2
    exit 1
fi

if ! command -v helm >/dev/null 2>&1; then
    echo "helm is required to publish the chart" >&2
    exit 1
fi

if ! command -v oras >/dev/null 2>&1; then
    echo "oras is required to publish Artifact Hub metadata" >&2
    exit 1
fi

chart_version="$1"
app_version="$2"
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
dist_dir="${repo_root}/dist/helm"
registry_host="${HELM_REGISTRY_HOST:-ghcr.io}"
default_registry_namespace="revaer/charts"
if [[ -n "${GITHUB_REPOSITORY_OWNER:-}" ]]; then
    default_registry_namespace="$(printf '%s' "${GITHUB_REPOSITORY_OWNER}" | tr '[:upper:]' '[:lower:]')/charts"
elif [[ -n "${GITHUB_REPOSITORY:-}" ]]; then
    default_registry_namespace="$(printf '%s' "${GITHUB_REPOSITORY%%/*}" | tr '[:upper:]' '[:lower:]')/charts"
fi
registry_namespace="${HELM_REGISTRY_NAMESPACE:-${default_registry_namespace}}"
chart_path="${dist_dir}/revaer-${chart_version}.tgz"
metadata_path="${dist_dir}/artifacthub-repo.yml"
provenance_path="${chart_path}.prov"
public_keyring_path="${dist_dir}/revaer-helm-public.gpg"
metadata_ref="${registry_host}/${registry_namespace}/revaer:artifacthub.io"
metadata_filename="$(basename "${metadata_path}")"
registry_username="${HELM_REGISTRY_USERNAME:-${HELM_API_KEY_ID:-}}"
registry_password="${HELM_REGISTRY_PASSWORD:-${HELM_API_KEY_SECRET:-}}"

if [[ -z "${registry_username}" || -z "${registry_password}" ]]; then
    if [[ "${registry_host}" == "ghcr.io" && -n "${GITHUB_TOKEN:-}" ]]; then
        registry_username="${GITHUB_ACTOR:-${GITHUB_REPOSITORY_OWNER:-}}"
        registry_password="${GITHUB_TOKEN}"
    fi
fi

if [[ -z "${registry_username}" || -z "${registry_password}" ]]; then
    echo "registry credentials are required via HELM_REGISTRY_USERNAME/HELM_REGISTRY_PASSWORD, HELM_API_KEY_ID/HELM_API_KEY_SECRET, or GITHUB_TOKEN for ghcr.io" >&2
    exit 1
fi

if [[ "${REVAER_HELM_SKIP_PACKAGE:-0}" != "1" ]]; then
    bash "${repo_root}/release/scripts/helm-package.sh" "${chart_version}" "${app_version}"
fi

if [[ ! -f "${chart_path}" ]]; then
    echo "missing packaged chart ${chart_path}" >&2
    exit 1
fi

if [[ ! -f "${metadata_path}" ]]; then
    echo "missing Artifact Hub metadata ${metadata_path}" >&2
    exit 1
fi

if [[ ! -f "${provenance_path}" ]]; then
    echo "missing chart provenance file ${provenance_path}" >&2
    exit 1
fi

if [[ ! -f "${public_keyring_path}" ]]; then
    echo "missing Helm public keyring ${public_keyring_path}" >&2
    exit 1
fi

helm verify "${chart_path}" --keyring "${public_keyring_path}"

printf '%s\n' "${registry_password}" | helm registry login "${registry_host}" \
    --username "${registry_username}" \
    --password-stdin
printf '%s\n' "${registry_password}" | oras login "${registry_host}" \
    --username "${registry_username}" \
    --password-stdin

helm push "${chart_path}" "oci://${registry_host}/${registry_namespace}"
(
    cd "${dist_dir}"
    oras push "${metadata_ref}" \
        --config /dev/null:application/vnd.cncf.artifacthub.config.v1+yaml \
        "${metadata_filename}:application/vnd.cncf.artifacthub.repository-metadata.layer.v1.yaml"
)
