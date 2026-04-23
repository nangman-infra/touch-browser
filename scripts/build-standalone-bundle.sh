#!/usr/bin/env bash
set -euo pipefail

normalize_platform() {
  case "$(uname -s)" in
    Linux) echo "linux" ;;
    Darwin) echo "macos" ;;
    *)
      echo "unsupported-platform" >&2
      exit 1
      ;;
  esac
}

normalize_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) echo "x86_64" ;;
    arm64 | aarch64) echo "arm64" ;;
    *) uname -m ;;
  esac
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "Required command not found: $1" >&2
    exit 1
  fi
}

write_wrapper() {
  cat >"${BUNDLE_ROOT}/bin/touch-browser" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

SOURCE_PATH="${BASH_SOURCE[0]}"
while [[ -h "${SOURCE_PATH}" ]]; do
  LINK_DIR="$(CDPATH= cd -- "$(dirname -- "${SOURCE_PATH}")" && pwd)"
  LINK_TARGET="$(readlink "${SOURCE_PATH}")"
  if [[ "${LINK_TARGET}" == /* ]]; then
    SOURCE_PATH="${LINK_TARGET}"
  else
    SOURCE_PATH="${LINK_DIR}/${LINK_TARGET}"
  fi
done

SCRIPT_DIR="$(CDPATH= cd -- "$(dirname -- "${SOURCE_PATH}")" && pwd)"
BUNDLE_ROOT="$(CDPATH= cd -- "${SCRIPT_DIR}/.." && pwd)"
RUNTIME_ROOT="${BUNDLE_ROOT}/runtime"

export TOUCH_BROWSER_RESOURCE_ROOT="${RUNTIME_ROOT}"
export TOUCH_BROWSER_NODE_EXECUTABLE="${RUNTIME_ROOT}/node/bin/node"
export TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH="${RUNTIME_ROOT}/models/evidence/embedding"
export TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH="${RUNTIME_ROOT}/models/evidence/nli"
export PLAYWRIGHT_BROWSERS_PATH="${RUNTIME_ROOT}/playwright-browsers"
export PATH="${RUNTIME_ROOT}/node/bin:${PATH}"

exec "${RUNTIME_ROOT}/touch-browser-bin" "$@"
EOF

  chmod +x "${BUNDLE_ROOT}/bin/touch-browser"
}

write_checksum() {
  if command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "${TARBALL_PATH}" | awk '{ print $1 }' >"${CHECKSUM_PATH}"
    return
  fi

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${TARBALL_PATH}" | awk '{ print $1 }' >"${CHECKSUM_PATH}"
    return
  fi

  echo "Need shasum or sha256sum to write checksum." >&2
  exit 1
}

require_command cargo
require_command pnpm
require_command node
require_command rsync
require_command tar

REPO_ROOT="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ "${1:-}" == "--" ]]; then
  shift
fi
VERSION="${1:-${TOUCH_BROWSER_BUNDLE_VERSION:-$(git -C "${REPO_ROOT}" describe --tags --always 2>/dev/null || echo dev)}}"
PLATFORM="${TOUCH_BROWSER_BUNDLE_PLATFORM:-$(normalize_platform)}"
ARCH="${TOUCH_BROWSER_BUNDLE_ARCH:-$(normalize_arch)}"
BUNDLE_PROFILE="${TOUCH_BROWSER_BUNDLE_PROFILE:-full}"
case "${BUNDLE_PROFILE}" in
  full) ;;
  slim)
    TOUCH_BROWSER_BUNDLE_SKIP_MODEL_WARMUP="${TOUCH_BROWSER_BUNDLE_SKIP_MODEL_WARMUP:-1}"
    TOUCH_BROWSER_BUNDLE_SKIP_PLAYWRIGHT_DOWNLOAD="${TOUCH_BROWSER_BUNDLE_SKIP_PLAYWRIGHT_DOWNLOAD:-1}"
    ;;
  *)
    echo "Unsupported TOUCH_BROWSER_BUNDLE_PROFILE: ${BUNDLE_PROFILE}. Use full or slim." >&2
    exit 1
    ;;
esac
BUNDLE_NAME="touch-browser-${VERSION}-${PLATFORM}-${ARCH}"
DIST_ROOT="${REPO_ROOT}/dist/standalone"
BUNDLE_ROOT="${DIST_ROOT}/${BUNDLE_NAME}"
RUNTIME_ROOT="${BUNDLE_ROOT}/runtime"
ARTIFACT_ROOT="${REPO_ROOT}/dist/release-assets"
NODE_BINARY="${TOUCH_BROWSER_NODE_BINARY:-$(command -v node)}"
NODE_PREFIX="$(CDPATH= cd -- "$(dirname -- "${NODE_BINARY}")/.." && pwd)"
PLAYWRIGHT_BROWSERS_ROOT="${RUNTIME_ROOT}/playwright-browsers"
EMBEDDING_MODEL_ROOT="${RUNTIME_ROOT}/models/evidence/embedding"
NLI_MODEL_ROOT="${RUNTIME_ROOT}/models/evidence/nli"
TARBALL_PATH="${ARTIFACT_ROOT}/${BUNDLE_NAME}.tar.gz"
CHECKSUM_PATH="${ARTIFACT_ROOT}/${BUNDLE_NAME}.sha256"

mkdir -p "${DIST_ROOT}" "${ARTIFACT_ROOT}"
rm -rf "${BUNDLE_ROOT}"
mkdir -p \
  "${BUNDLE_ROOT}/bin" \
  "${RUNTIME_ROOT}/adapters/playwright" \
  "${RUNTIME_ROOT}/contracts/generated" \
  "${RUNTIME_ROOT}/integrations/mcp/bridge" \
  "${RUNTIME_ROOT}/scripts/lib"

(
  cd "${REPO_ROOT}"
  cargo build --release -p touch-browser-cli
  pnpm exec tsc -p adapters/playwright/tsconfig.runtime.json
)

cp "${REPO_ROOT}/target/release/touch-browser" "${RUNTIME_ROOT}/touch-browser-bin"
chmod +x "${RUNTIME_ROOT}/touch-browser-bin"

rsync -a "${NODE_PREFIX}/" "${RUNTIME_ROOT}/node/"
rsync -a "${REPO_ROOT}/node_modules/" "${RUNTIME_ROOT}/node_modules/"
rsync -a "${REPO_ROOT}/adapters/playwright/dist-runtime/src/" "${RUNTIME_ROOT}/adapters/playwright/"
rsync -a "${REPO_ROOT}/contracts/generated/" "${RUNTIME_ROOT}/contracts/generated/"
rsync -a "${REPO_ROOT}/integrations/mcp/bridge/" "${RUNTIME_ROOT}/integrations/mcp/bridge/"

cp "${REPO_ROOT}/scripts/evidence-embedding-runner.mjs" "${RUNTIME_ROOT}/scripts/evidence-embedding-runner.mjs"
cp "${REPO_ROOT}/scripts/evidence-nli-runner.mjs" "${RUNTIME_ROOT}/scripts/evidence-nli-runner.mjs"
cp "${REPO_ROOT}/scripts/touch-browser-mcp-bridge.mjs" "${RUNTIME_ROOT}/scripts/touch-browser-mcp-bridge.mjs"
cp "${REPO_ROOT}/scripts/lib/model-runner.mjs" "${RUNTIME_ROOT}/scripts/lib/model-runner.mjs"
cp "${REPO_ROOT}/scripts/lib/serve-rpc-client.mjs" "${RUNTIME_ROOT}/scripts/lib/serve-rpc-client.mjs"
cp "${REPO_ROOT}/scripts/lib/shell-command.mjs" "${RUNTIME_ROOT}/scripts/lib/shell-command.mjs"
cp "${REPO_ROOT}/scripts/lib/touch-browser-command.mjs" "${RUNTIME_ROOT}/scripts/lib/touch-browser-command.mjs"
cp "${REPO_ROOT}/scripts/install-standalone-bundle.sh" "${BUNDLE_ROOT}/install.sh"
cp "${REPO_ROOT}/scripts/uninstall-standalone-bundle.sh" "${BUNDLE_ROOT}/uninstall.sh"
chmod +x "${BUNDLE_ROOT}/install.sh"
chmod +x "${BUNDLE_ROOT}/uninstall.sh"

if [[ "${TOUCH_BROWSER_BUNDLE_SKIP_MODEL_WARMUP:-0}" == "1" ]]; then
  mkdir -p "${EMBEDDING_MODEL_ROOT}" "${NLI_MODEL_ROOT}"
else
  "${NODE_BINARY}" "${REPO_ROOT}/scripts/evidence-embedding-runner.mjs" \
    --warmup \
    --allow-download \
    --model-root "${EMBEDDING_MODEL_ROOT}"

  "${NODE_BINARY}" "${REPO_ROOT}/scripts/evidence-nli-runner.mjs" \
    --warmup \
    --allow-download \
    --model-root "${NLI_MODEL_ROOT}"
fi

if [[ "${TOUCH_BROWSER_BUNDLE_SKIP_PLAYWRIGHT_DOWNLOAD:-0}" == "1" ]]; then
  mkdir -p "${PLAYWRIGHT_BROWSERS_ROOT}"
else
  (
    cd "${REPO_ROOT}"
    PLAYWRIGHT_BROWSERS_PATH="${PLAYWRIGHT_BROWSERS_ROOT}" \
      pnpm exec playwright install chromium
  )
fi

cp "${REPO_ROOT}/README.md" "${BUNDLE_ROOT}/README.md"
cp "${REPO_ROOT}/LICENSE" "${BUNDLE_ROOT}/LICENSE"

write_wrapper

rm -f "${TARBALL_PATH}" "${CHECKSUM_PATH}"
tar -C "${DIST_ROOT}" -czf "${TARBALL_PATH}" "${BUNDLE_NAME}"
write_checksum

echo "Standalone bundle created:"
echo "  profile: ${BUNDLE_PROFILE}"
echo "  ${TARBALL_PATH}"
echo "  ${CHECKSUM_PATH}"
