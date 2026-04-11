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

resolve_data_root() {
  local configured_root="${TOUCH_BROWSER_DATA_ROOT:-${HOME}/.touch-browser}"
  configured_root="$(expand_tilde "${configured_root}")"
  mkdir -p "${configured_root}"
  cd -- "${configured_root}" && pwd
}

remove_if_exists() {
  local target="${1}"
  if [[ -L "${target}" || -f "${target}" ]]; then
    rm -f "${target}"
    echo "removed ${target}"
    return
  fi
  if [[ -d "${target}" ]]; then
    rm -rf "${target}"
    echo "removed ${target}"
  fi
}

remove_empty_parents() {
  local current="${1}"
  local stop="${2:-}"
  while [[ -n "${current}" && "${current}" != "${stop}" ]]; do
    if [[ -d "${current}" ]] && [[ -z "$(ls -A "${current}" 2>/dev/null)" ]]; then
      rmdir "${current}" 2>/dev/null || break
      current="$(dirname "${current}")"
      continue
    fi
    break
  done
}

PURGE_DATA=0
PURGE_ALL=0
CONFIRMED=0

while [[ $# -gt 0 ]]; do
  case "${1}" in
    --purge-data)
      PURGE_DATA=1
      shift
      ;;
    --purge-all)
      PURGE_ALL=1
      PURGE_DATA=1
      shift
      ;;
    --yes)
      CONFIRMED=1
      shift
      ;;
    *)
      echo "Unknown option for uninstall.sh: ${1}" >&2
      exit 1
      ;;
  esac
done

if [[ "${CONFIRMED}" != "1" ]]; then
  echo "uninstall is destructive. Re-run with --yes." >&2
  exit 1
fi

DATA_ROOT="$(resolve_data_root)"
INSTALL_ROOT="${DATA_ROOT}/install"
MANIFEST_ENV="${INSTALL_ROOT}/install-manifest.env"
COMMAND_LINK=""

if [[ -f "${MANIFEST_ENV}" ]]; then
  # shellcheck disable=SC1090
  source "${MANIFEST_ENV}"
  COMMAND_LINK="${TOUCH_BROWSER_COMMAND_LINK:-}"
fi

if [[ -n "${COMMAND_LINK}" ]]; then
  remove_if_exists "${COMMAND_LINK}"
  remove_empty_parents "$(dirname "${COMMAND_LINK}")" "${HOME}"
fi

remove_if_exists "${INSTALL_ROOT}"

if [[ "${PURGE_DATA}" == "1" ]]; then
  remove_if_exists "${DATA_ROOT}/browser-search"
  remove_if_exists "${DATA_ROOT}/pilot"
fi

if [[ "${PURGE_ALL}" == "1" ]]; then
  remove_if_exists "${DATA_ROOT}/models"
fi

remove_empty_parents "${DATA_ROOT}" "${HOME}"
