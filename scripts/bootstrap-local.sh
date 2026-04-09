#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v rustup >/dev/null 2>&1; then
  echo "rustup is required." >&2
  exit 1
fi

if ! command -v pnpm >/dev/null 2>&1; then
  echo "pnpm is required." >&2
  exit 1
fi

rustup component add rustfmt clippy
mkdir -p contracts/generated/ts contracts/generated/rust
pnpm install
./scripts/setup-models.sh
pnpm run contracts:check
pnpm run contracts:manifest
pnpm exec playwright install chromium
cargo build -q --workspace
pnpm typecheck
