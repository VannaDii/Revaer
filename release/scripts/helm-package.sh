#!/usr/bin/env bash

set -euo pipefail

if [[ "$#" -ne 2 ]]; then
    echo "usage: $0 <chart-version> <app-version>" >&2
    exit 1
fi

if ! command -v helm >/dev/null 2>&1; then
    echo "helm is required to package the chart" >&2
    exit 1
fi

chart_version="$1"
app_version="$2"
sign_chart="${REVAER_HELM_SIGN:-1}"
repo_root="$(cd "$(dirname "$0")/../.." && pwd)"
chart_root="${repo_root}/charts/revaer"
dist_dir="${repo_root}/dist/helm"
public_key_asset="revaer-helm-public.asc"
public_keyring_asset="revaer-helm-public.gpg"
metadata_template="${chart_root}/artifacthub-repo.yml"
release_repository="${REVAER_RELEASE_REPOSITORY:-${GITHUB_REPOSITORY:-VannaDii/Revaer}}"
release_asset_url="https://github.com/${release_repository}/releases/download/${app_version}/${public_key_asset}"
lint_database_url="${REVAER_HELM_LINT_DATABASE_URL:-postgres://revaer:revaer@postgres.default.svc.cluster.local:5432/revaer}"

rm -rf "${dist_dir}"
mkdir -p "${dist_dir}"

chart_copy_dir="$(mktemp -d)"
gnupg_dir="$(mktemp -d)"
cleanup() {
    rm -rf "${chart_copy_dir}" "${gnupg_dir}"
    return 0
}
trap cleanup EXIT

yaml_quote() {
    local value="$1"

    case "${value}" in
        *$'\n'*|*$'\r'*)
            echo "Artifact Hub owner metadata must not contain newlines" >&2
            exit 1
            ;;
        *)
            ;;
    esac

    printf "'%s'" "$(printf '%s' "${value}" | sed "s/'/''/g")"
    return 0
}

append_repository_id() {
    local metadata_file="$1"
    local repository_id="$2"

    if grep -Eq '^[[:space:]]*repositoryID:' "${metadata_file}"; then
        return 0
    fi

    printf 'repositoryID: %s\n' "${repository_id}" >> "${metadata_file}"
    return 0
}

append_owner() {
    local metadata_file="$1"
    local owner_name="$2"
    local owner_email="$3"

    if [[ -z "${owner_name}" || -z "${owner_email}" ]]; then
        return 0
    fi

    if grep -Eq '^[[:space:]]*owners:' "${metadata_file}"; then
        return 0
    fi

    cat >> "${metadata_file}" <<EOF
owners:
  - name: $(yaml_quote "${owner_name}")
    email: $(yaml_quote "${owner_email}")
EOF
    return 0
}

cp -R "${chart_root}" "${chart_copy_dir}/revaer"
chart_yaml="${chart_copy_dir}/revaer/Chart.yaml"
metadata_output="${dist_dir}/artifacthub-repo.yml"
cp "${metadata_template}" "${metadata_output}"
release_owner="$(printf '%s' "${release_repository%%/*}" | tr '[:upper:]' '[:lower:]')"
image_repository="${REVAER_HELM_IMAGE_REPOSITORY:-ghcr.io/${release_owner}/revaer}"
owner_name="${ARTIFACTHUB_OWNER_NAME:-}"
owner_email="${ARTIFACTHUB_OWNER_EMAIL:-}"

prerelease="false"
if [[ "${chart_version}" == *-* ]]; then
    prerelease="true"
fi

release_annotations="$(cat <<EOF
  artifacthub.io/prerelease: "${prerelease}"
  artifacthub.io/images: |
    - name: revaer
      image: ${image_repository}:${app_version}
      platforms:
        - linux/amd64
        - linux/arm64
EOF
)"

if [[ "${sign_chart}" == "1" ]]; then
    if ! command -v gpg >/dev/null 2>&1; then
        echo "gpg is required to sign the chart" >&2
        exit 1
    fi

    if [[ -z "${HELM_GPG_PRIVATE:-}" || -z "${HELM_GPG_PUBLIC:-}" ]]; then
        echo "HELM_GPG_PRIVATE and HELM_GPG_PUBLIC are required when REVAER_HELM_SIGN=1" >&2
        exit 1
    fi

    chmod 700 "${gnupg_dir}"
    export GNUPGHOME="${gnupg_dir}"

    printf '%s\n' "${HELM_GPG_PUBLIC}" > "${dist_dir}/${public_key_asset}"
    chmod 644 "${dist_dir}/${public_key_asset}"

    printf '%s\n' "${HELM_GPG_PUBLIC}" | gpg --batch --yes --import >/dev/null 2>&1
    printf '%s\n' "${HELM_GPG_PRIVATE}" | gpg --batch --yes --import >/dev/null 2>&1
    gpg --batch --export > "${dist_dir}/${public_keyring_asset}"
    secret_keyring="${gnupg_dir}/secring.gpg"
    (
        umask 077
        gpg --batch --export-secret-keys > "${secret_keyring}"
    )
    chmod 600 "${secret_keyring}"

    signing_uid="$(gpg --batch --list-secret-keys --with-colons | awk -F: '/^uid:/ {print $10; exit}')"
    fingerprint="$(gpg --batch --list-secret-keys --with-colons --fingerprint | awk -F: '/^fpr:/ {print $10; exit}')"
    if [[ -z "${owner_name}" ]]; then
        owner_name="$(printf '%s' "${signing_uid}" | sed -E 's/ <[^>]+>$//')"
    fi
    if [[ -z "${owner_email}" ]]; then
        owner_email="$(printf '%s' "${signing_uid}" | sed -nE 's/.*<([^>]+)>.*/\1/p')"
    fi

    if [[ -z "${signing_uid}" || -z "${fingerprint}" ]]; then
        echo "failed to resolve imported GPG signing identity" >&2
        exit 1
    fi

    release_annotations="$(cat <<EOF
${release_annotations}
  artifacthub.io/signKey: |
    fingerprint: ${fingerprint}
    url: ${release_asset_url}
EOF
)"

    awk -v replacement="${release_annotations}" '
        /__RELEASE_HELM_ANNOTATIONS__/ {
            print replacement
            next
        }
        { print }
    ' "${chart_root}/Chart.yaml" > "${chart_yaml}"

    if [[ -n "${ARTIFACTHUB_REPOSITORY_ID:-}" ]]; then
        append_repository_id "${metadata_output}" "${ARTIFACTHUB_REPOSITORY_ID}"
    fi
    append_owner "${metadata_output}" "${owner_name}" "${owner_email}"

    helm lint "${chart_copy_dir}/revaer" --set "database.url=${lint_database_url}"
    helm package "${chart_copy_dir}/revaer" \
        --destination "${dist_dir}" \
        --version "${chart_version}" \
        --app-version "${app_version}" \
        --sign \
        --key "${signing_uid}" \
        --keyring "${secret_keyring}"
    helm verify "${dist_dir}/revaer-${chart_version}.tgz" --keyring "${dist_dir}/${public_keyring_asset}"
else
    awk -v replacement="${release_annotations}" '
        /__RELEASE_HELM_ANNOTATIONS__/ {
            print replacement
            next
        }
        { print }
    ' "${chart_root}/Chart.yaml" > "${chart_yaml}"
    if [[ -n "${ARTIFACTHUB_REPOSITORY_ID:-}" ]]; then
        append_repository_id "${metadata_output}" "${ARTIFACTHUB_REPOSITORY_ID}"
    fi
    append_owner "${metadata_output}" "${owner_name}" "${owner_email}"
    helm lint "${chart_copy_dir}/revaer" --set "database.url=${lint_database_url}"
    helm package "${chart_copy_dir}/revaer" \
        --destination "${dist_dir}" \
        --version "${chart_version}" \
        --app-version "${app_version}"
fi
