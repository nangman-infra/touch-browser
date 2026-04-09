#!/usr/bin/env bash
set -euo pipefail

EMBEDDING_MODEL_ROOT="${HOME}/.touch-browser/models/evidence/embedding"
EMBEDDING_READY_PATH="${EMBEDDING_MODEL_ROOT}/.ready.json"
NLI_MODEL_ROOT="${HOME}/.touch-browser/models/evidence/nli"
NLI_READY_PATH="${NLI_MODEL_ROOT}/.ready.json"

if [[ "${TOUCH_BROWSER_SKIP_MODEL_DOWNLOAD:-0}" == "1" ]]; then
  echo "Skipping semantic model download because TOUCH_BROWSER_SKIP_MODEL_DOWNLOAD=1"
  exit 0
fi

if ! command -v node >/dev/null 2>&1; then
  echo "node is required to prepare semantic models." >&2
  exit 1
fi

if ! node --input-type=module -e "import('@huggingface/transformers').catch(() => process.exit(1))" >/dev/null 2>&1; then
  echo "Run 'pnpm install' before preparing semantic models." >&2
  exit 1
fi

mkdir -p "${EMBEDDING_MODEL_ROOT}"
mkdir -p "${NLI_MODEL_ROOT}"

if [[ -f "${EMBEDDING_READY_PATH}" ]]; then
  echo "Embedding model cache already present at ${EMBEDDING_MODEL_ROOT}"
else
  echo "Preparing default embedding model cache under ${EMBEDDING_MODEL_ROOT}"
  node scripts/evidence-embedding-runner.mjs \
    --warmup \
    --allow-download \
    --model-root "${EMBEDDING_MODEL_ROOT}"

  echo "Installed embedding model cache at ${EMBEDDING_MODEL_ROOT}"
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
