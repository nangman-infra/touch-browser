#!/usr/bin/env bash
set -euo pipefail

MODEL_ROOT="${HOME}/.touch-browser/models/evidence/fasttext"
MODEL_NAME="cc.en.300.bin"
MODEL_PATH="${MODEL_ROOT}/${MODEL_NAME}"
MODEL_URL="https://dl.fbaipublicfiles.com/fasttext/vectors-crawl/${MODEL_NAME}.gz"

if [[ "${TOUCH_BROWSER_SKIP_MODEL_DOWNLOAD:-0}" == "1" ]]; then
  echo "Skipping semantic model download because TOUCH_BROWSER_SKIP_MODEL_DOWNLOAD=1"
  exit 0
fi

if ! command -v curl >/dev/null 2>&1; then
  echo "curl is required to download semantic models." >&2
  exit 1
fi

if ! command -v gzip >/dev/null 2>&1; then
  echo "gzip is required to unpack semantic models." >&2
  exit 1
fi

mkdir -p "${MODEL_ROOT}"

if [[ -f "${MODEL_PATH}" ]]; then
  echo "Semantic model already present at ${MODEL_PATH}"
  exit 0
fi

TEMP_ARCHIVE="$(mktemp "${TMPDIR:-/tmp}/touch-browser-fasttext-XXXXXX.gz")"
trap 'rm -f "${TEMP_ARCHIVE}"' EXIT

echo "Downloading default semantic model to ${MODEL_PATH}"
curl --fail --location --progress-bar --output "${TEMP_ARCHIVE}" "${MODEL_URL}"
gzip --decompress --stdout "${TEMP_ARCHIVE}" > "${MODEL_PATH}"
chmod 0644 "${MODEL_PATH}"

echo "Installed semantic model at ${MODEL_PATH}"
