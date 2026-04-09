#!/usr/bin/env bash
set -euo pipefail

MODEL_ROOT="${HOME}/.touch-browser/models/evidence/fasttext"
MODEL_NAME="cc.en.300.bin"
MODEL_PATH="${MODEL_ROOT}/${MODEL_NAME}"
MODEL_URL="https://dl.fbaipublicfiles.com/fasttext/vectors-crawl/${MODEL_NAME}.gz"
NLI_MODEL_ROOT="${HOME}/.touch-browser/models/evidence/nli"
NLI_READY_PATH="${NLI_MODEL_ROOT}/.ready.json"

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

if ! command -v node >/dev/null 2>&1; then
  echo "node is required to prepare NLI semantic models." >&2
  exit 1
fi

mkdir -p "${MODEL_ROOT}"
mkdir -p "${NLI_MODEL_ROOT}"

if [[ -f "${MODEL_PATH}" ]]; then
  echo "Semantic model already present at ${MODEL_PATH}"
else
  TEMP_ARCHIVE="$(mktemp "${TMPDIR:-/tmp}/touch-browser-fasttext-XXXXXX.gz")"
  trap 'rm -f "${TEMP_ARCHIVE}"' EXIT

  echo "Downloading default semantic model to ${MODEL_PATH}"
  curl --fail --location --progress-bar --output "${TEMP_ARCHIVE}" "${MODEL_URL}"
  gzip --decompress --stdout "${TEMP_ARCHIVE}" > "${MODEL_PATH}"
  chmod 0644 "${MODEL_PATH}"

  echo "Installed semantic model at ${MODEL_PATH}"
fi

if [[ -f "${NLI_READY_PATH}" ]]; then
  echo "NLI model cache already present at ${NLI_MODEL_ROOT}"
  exit 0
fi

echo "Preparing default NLI model cache under ${NLI_MODEL_ROOT}"
node scripts/evidence-nli-runner.mjs \
  --warmup \
  --allow-download \
  --model-root "${NLI_MODEL_ROOT}"

echo "Installed NLI model cache at ${NLI_MODEL_ROOT}"
