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

BUNDLE_ROOT="$(resolve_bundle_root "${1:-}")"
WRAPPER_PATH="${BUNDLE_ROOT}/bin/touch-browser"

if [[ ! -x "${WRAPPER_PATH}" ]]; then
  echo "touch-browser wrapper not found at ${WRAPPER_PATH}" >&2
  exit 1
fi

INSTALL_DIR="$(pick_install_dir)"
TARGET_PATH="${INSTALL_DIR}/touch-browser"

ln -sfn "${WRAPPER_PATH}" "${TARGET_PATH}"

echo "Installed touch-browser command:"
echo "  ${TARGET_PATH} -> ${WRAPPER_PATH}"

if path_contains "${INSTALL_DIR}"; then
  echo "PATH already includes ${INSTALL_DIR}. You can now run:"
  echo "  touch-browser telemetry-summary"
else
  echo "Add this directory to PATH, then open a new shell:"
  echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
fi
