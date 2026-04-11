#!/usr/bin/env bash
set -euo pipefail

expand_tilde() {
  local value="${1:-}"
  if [[ -z "${value}" ]]; then
    return
  fi
  if [[ "${value}" == "~" ]]; then
    printf '%s\n' "${HOME}"
    return
  fi
  if [[ "${value}" == ~/* ]]; then
    printf '%s/%s\n' "${HOME}" "${value#~/}"
    return
  fi
  printf '%s\n' "${value}"
}

json_escape() {
  local value="${1:-}"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  printf '%s' "${value}"
}

shell_quote() {
  local value="${1:-}"
  printf "'%s'" "${value//\'/\'\"\'\"\'}"
}

resolve_bundle_root() {
  if [[ $# -gt 0 && -n "${1}" ]]; then
    cd -- "$(expand_tilde "${1}")" && pwd
    return
  fi

  local script_dir
  script_dir="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

  if [[ -x "${script_dir}/bin/touch-browser" ]]; then
    printf '%s\n' "${script_dir}"
    return
  fi

  if [[ -x "${script_dir}/../bin/touch-browser" ]]; then
    cd -- "${script_dir}/.." && pwd
    return
  fi

  echo "Could not locate a standalone touch-browser bundle root." >&2
  exit 1
}

path_contains() {
  local needle="${1}"
  local segment
  IFS=':' read -r -a path_segments <<< "${PATH:-}"
  for segment in "${path_segments[@]}"; do
    if [[ "${segment}" == "${needle}" ]]; then
      return 0
    fi
  done
  return 1
}

pick_install_dir() {
  local configured_dir="${TOUCH_BROWSER_INSTALL_DIR:-}"
  if [[ -n "${configured_dir}" ]]; then
    configured_dir="$(expand_tilde "${configured_dir}")"
    mkdir -p "${configured_dir}"
    printf '%s\n' "${configured_dir}"
    return
  fi

  for candidate in "${HOME}/.local/bin" "${HOME}/bin" "/usr/local/bin" "/opt/homebrew/bin"; do
    if path_contains "${candidate}" && [[ -d "${candidate}" && -w "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return
    fi
  done

  for candidate in "${HOME}/.local/bin" "${HOME}/bin" "/usr/local/bin" "/opt/homebrew/bin"; do
    mkdir -p "${candidate}" 2>/dev/null || true
    if [[ -d "${candidate}" && -w "${candidate}" ]]; then
      printf '%s\n' "${candidate}"
      return
    fi
  done

  echo "Could not find a writable install directory for touch-browser." >&2
  exit 1
}

resolve_data_root() {
  local configured_root="${TOUCH_BROWSER_DATA_ROOT:-${HOME}/.touch-browser}"
  configured_root="$(expand_tilde "${configured_root}")"
  mkdir -p "${configured_root}"
  cd -- "${configured_root}" && pwd
}

parse_bundle_identity() {
  local bundle_name="${1}"
  if [[ "${bundle_name}" =~ ^touch-browser-(.+)-(linux|macos)-(x86_64|arm64)$ ]]; then
    BUNDLE_VERSION="${BASH_REMATCH[1]}"
    BUNDLE_PLATFORM="${BASH_REMATCH[2]}"
    BUNDLE_ARCH="${BASH_REMATCH[3]}"
    return
  fi

  echo "Unsupported standalone bundle name: ${bundle_name}" >&2
  exit 1
}

copy_bundle_to_managed_root() {
  mkdir -p "${VERSIONS_ROOT}"
  if [[ -d "${MANAGED_BUNDLE_ROOT}" ]]; then
    rm -rf "${MANAGED_BUNDLE_ROOT}"
  fi
  mkdir -p "${MANAGED_BUNDLE_ROOT}"
  cp -R "${BUNDLE_ROOT}/." "${MANAGED_BUNDLE_ROOT}/"
}

write_json_manifest() {
  cat >"${INSTALL_MANIFEST_PATH}" <<EOF
{
  "schemaVersion": 1,
  "repository": "nangman-infra/touch-browser",
  "version": "$(json_escape "${BUNDLE_VERSION}")",
  "platform": "$(json_escape "${BUNDLE_PLATFORM}")",
  "arch": "$(json_escape "${BUNDLE_ARCH}")",
  "bundleName": "$(json_escape "${BUNDLE_NAME}")",
  "dataRoot": "$(json_escape "${DATA_ROOT}")",
  "installRoot": "$(json_escape "${INSTALL_ROOT}")",
  "managedBundleRoot": "$(json_escape "${MANAGED_BUNDLE_ROOT}")",
  "currentSymlink": "$(json_escape "${CURRENT_LINK}")",
  "commandLink": "$(json_escape "${TARGET_PATH}")",
  "installedAt": "$(json_escape "${INSTALLED_AT}")"
}
EOF
}

write_shell_manifest() {
  cat >"${INSTALL_MANIFEST_ENV_PATH}" <<EOF
TOUCH_BROWSER_INSTALL_SCHEMA_VERSION=1
TOUCH_BROWSER_INSTALL_REPOSITORY=$(shell_quote "nangman-infra/touch-browser")
TOUCH_BROWSER_INSTALL_VERSION=$(shell_quote "${BUNDLE_VERSION}")
TOUCH_BROWSER_INSTALL_PLATFORM=$(shell_quote "${BUNDLE_PLATFORM}")
TOUCH_BROWSER_INSTALL_ARCH=$(shell_quote "${BUNDLE_ARCH}")
TOUCH_BROWSER_BUNDLE_NAME=$(shell_quote "${BUNDLE_NAME}")
TOUCH_BROWSER_DATA_ROOT=$(shell_quote "${DATA_ROOT}")
TOUCH_BROWSER_INSTALL_ROOT=$(shell_quote "${INSTALL_ROOT}")
TOUCH_BROWSER_MANAGED_BUNDLE_ROOT=$(shell_quote "${MANAGED_BUNDLE_ROOT}")
TOUCH_BROWSER_CURRENT_LINK=$(shell_quote "${CURRENT_LINK}")
TOUCH_BROWSER_COMMAND_LINK=$(shell_quote "${TARGET_PATH}")
TOUCH_BROWSER_INSTALLED_AT=$(shell_quote "${INSTALLED_AT}")
EOF
}

BUNDLE_ROOT="$(resolve_bundle_root "${1:-}")"
BUNDLE_NAME="$(basename "${BUNDLE_ROOT}")"
parse_bundle_identity "${BUNDLE_NAME}"

WRAPPER_PATH="${BUNDLE_ROOT}/bin/touch-browser"
if [[ ! -x "${WRAPPER_PATH}" ]]; then
  echo "touch-browser wrapper not found at ${WRAPPER_PATH}" >&2
  exit 1
fi

DATA_ROOT="$(resolve_data_root)"
INSTALL_ROOT="${DATA_ROOT}/install"
VERSIONS_ROOT="${INSTALL_ROOT}/versions"
CURRENT_LINK="${INSTALL_ROOT}/current"
INSTALL_MANIFEST_PATH="${INSTALL_ROOT}/install-manifest.json"
INSTALL_MANIFEST_ENV_PATH="${INSTALL_ROOT}/install-manifest.env"
MANAGED_BUNDLE_ROOT="${VERSIONS_ROOT}/${BUNDLE_NAME}"
INSTALL_DIR="$(pick_install_dir)"
TARGET_PATH="${INSTALL_DIR}/touch-browser"
INSTALLED_AT="$(date -u +"%Y-%m-%dT%H:%M:%SZ")"

mkdir -p "${INSTALL_ROOT}"
copy_bundle_to_managed_root
ln -sfn "${MANAGED_BUNDLE_ROOT}" "${CURRENT_LINK}"
ln -sfn "${CURRENT_LINK}/bin/touch-browser" "${TARGET_PATH}"
write_json_manifest
write_shell_manifest

echo "Installed touch-browser command:"
echo "  ${TARGET_PATH} -> ${CURRENT_LINK}/bin/touch-browser"
echo "Managed bundle root:"
echo "  ${MANAGED_BUNDLE_ROOT}"
echo "Install manifest:"
echo "  ${INSTALL_MANIFEST_PATH}"

if path_contains "${INSTALL_DIR}"; then
  echo "PATH already includes ${INSTALL_DIR}. You can now run:"
  echo "  touch-browser telemetry-summary"
else
  echo "Add this directory to PATH, then open a new shell:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
