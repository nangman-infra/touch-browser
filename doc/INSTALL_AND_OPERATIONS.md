# Install And Operations

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-14`
- Scope: `local bootstrap, validation, and day-to-day operation`

## 1. Distribution Paths

### npm MCP package

Primary local-host MCP distribution:

```bash
npx -y @nangman-infra/touch-browser-mcp
```

Or install it globally:

```bash
npm install -g @nangman-infra/touch-browser-mcp
touch-browser-mcp
```

This package is the recommended path for local MCP hosts. It is scoped to public docs and research web workflows, stays headless over MCP, chooses search engines automatically, and hands challenge/auth/MFA cases to supervised recovery instead of exposing `--headed` or engine controls through MCP.

On first launch, it downloads the matching standalone runtime bundle from GitHub Releases, verifies the published `.sha256`, installs it under `~/.touch-browser/npm-mcp/versions/`, and then starts `touch-browser mcp`.

### Standalone release bundle

Tagged `v*` pushes build standalone macOS and Linux tarballs through `.github/workflows/release-standalone.yml`.

Each bundle contains:

- `bin/touch-browser`
- `runtime/touch-browser-bin`
- bundled Node runtime, Playwright adapter, and semantic runner scripts
- bundled semantic model cache

After downloading and unpacking a release asset from [GitHub Releases](https://github.com/nangman-infra/touch-browser/releases), run:

```bash
./touch-browser-<version>-<platform>-<arch>/install.sh
touch-browser telemetry-summary
touch-browser update --check
```

This installed `touch-browser` command is the official user-facing runtime path for CLI, operations, offline fallback, and manual verification.
The installer copies the bundle into a managed location under `~/.touch-browser/install/versions/<bundle-name>`, points `~/.touch-browser/install/current` at the active version, and links the PATH command to `~/.touch-browser/install/current/bin/touch-browser`.

### Build the standalone bundle locally

```bash
pnpm install --frozen-lockfile
pnpm run build:standalone-bundle -- v0.1.0-rc1

# Output:
# dist/standalone/touch-browser-v0.1.0-rc1-<platform>-<arch>/

# Install the command into PATH
./dist/standalone/touch-browser-v0.1.0-rc1-<platform>-<arch>/install.sh

# Then use the installed command directly
touch-browser telemetry-summary
touch-browser update --check
```

### Build from source

Prerequisites:

- [rustup](https://rustup.rs)
- Node.js 18+
- `pnpm`

Bootstrap:

```bash
bash scripts/bootstrap-local.sh
cargo build --release -p touch-browser-cli
pnpm run build:standalone-bundle -- local-dev
./dist/standalone/touch-browser-local-dev-<platform>-<arch>/install.sh
```

`bootstrap-local.sh` also installs the default semantic models under:
- `~/.touch-browser/models/evidence/embedding`
- `~/.touch-browser/models/evidence/nli`

Use `TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH` or `TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH`
only when you need to override the default model locations.

Manual setup:

```bash
rustup component add rustfmt clippy
pnpm install
mkdir -p contracts/generated/ts contracts/generated/rust
pnpm run contracts:check
pnpm run contracts:manifest
pnpm exec playwright install chromium
cargo build -q --workspace
pnpm typecheck
pnpm test
```

Build-from-source is a contributor path. The runtime examples below still assume the installed `touch-browser` command so the user-facing path stays identical between release and local verification.

## 2. Core Commands

### First-run proof path

Use this path to verify a fresh install from the user point of view:

```bash
touch-browser open https://www.iana.org/help/example-domains --browser --session-file /tmp/tb-first-run.json
touch-browser session-read --session-file /tmp/tb-first-run.json --main-only
touch-browser session-extract --session-file /tmp/tb-first-run.json \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."
touch-browser session-synthesize --session-file /tmp/tb-first-run.json --format markdown
touch-browser session-close --session-file /tmp/tb-first-run.json
```

Installed search defaults to engine trust profiles under the active data root:

- Google: `~/.touch-browser/browser-search/profiles/google-default`
- Brave: `~/.touch-browser/browser-search/profiles/brave-default`
- profile state metadata: `~/.touch-browser/browser-search/<engine>.profile-state.json`
- saved search sessions: `~/.touch-browser/browser-search/<engine>.search-session.json`

Read a real page:

```bash
touch-browser read-view https://www.iana.org/help/example-domains
```

For navigation-heavy pages, add `--main-only` to keep the Markdown output centered on the primary content region.

Generate the low-token agent view:

```bash
touch-browser compact-view https://www.iana.org/help/example-domains
```

Extract evidence with an optional verifier hook:

```bash
touch-browser extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes." \
  --verifier-command 'node scripts/example-verifier.mjs'
```

Evidence operating rule of thumb:

- `evidence-supported` + `confidenceBand=high` + `reviewRecommended=false`: safe enough for direct reuse inside the curated pilot domains
- `confidenceBand=medium`: reuse only for lower-impact answers or after additional review
- `confidenceBand=review` or `reviewRecommended=true`: run a verifier or browse another page before answering
- `needs-more-browsing`: open a more specific page
- `contradicted`: reuse only together with `verdictExplanation` and the attached snippets

Render a multi-page session as Markdown:

```bash
touch-browser session-synthesize --session-file /tmp/tb-session.json --format markdown
```

Serve daemon:

```bash
touch-browser serve
```

Check for a new standalone release:

```bash
touch-browser update --check
```

Install the latest matching release into the managed install:

```bash
touch-browser update
```

Install a specific release tag into the managed install:

```bash
touch-browser update --version v0.1.1
```

Remove the managed install but keep user data:

```bash
touch-browser uninstall --yes
```

Remove the managed install and all stored data:

```bash
touch-browser uninstall --purge-all --yes
```

Installed MCP package:

```bash
npx -y @nangman-infra/touch-browser-mcp
```

Installed standalone MCP bridge:

```bash
touch-browser mcp
```

MCP bridge (repo checkout integration asset):

```bash
node integrations/mcp/bridge/index.mjs
```

Self-hosted pilot package:

```bash
docker build -f deploy/Dockerfile -t touch-browser:pilot .
docker compose -f deploy/docker-compose.pilot.yml up --build
pnpm run pilot:healthcheck
```

## 3. Public Proof Runs

```bash
pnpm run fixtures:public-web
pnpm run pilot:public-reference-workflow
pnpm run pilot:real-user-research
```

## 4. Operational Checks

- `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm typecheck`
- `pnpm test`
- `pnpm run pilot:healthcheck`
- optionally `pnpm run fixtures:public-web`
- optionally `pnpm run pilot:public-reference-workflow`
- optionally `pnpm run fixtures:safety`

## 5. Troubleshooting

- browser launch failure:
  - run `pnpm exec playwright install chromium`
- Rust command not found:
  - confirm `rustup` and `cargo` are on the shell `PATH`
- public benchmark failure:
  - check network access and remote site availability
- MCP bridge failure:
  - if you are using the npm MCP package, start with `npx -y @nangman-infra/touch-browser-mcp doctor`
  - the npm package installs its managed runtime under `~/.touch-browser/npm-mcp/versions/`
  - verify `touch-browser serve` works on its own
  - the bridge prefers `TOUCH_BROWSER_SERVE_COMMAND`, then `TOUCH_BROWSER_SERVE_BINARY`, then an installed or packaged `touch-browser` binary, then repo-local `target/{release,debug}` binaries
  - if no binary can be resolved, install a standalone bundle with `install.sh`; source-checkout operators can also build once so `target/release` or `target/debug` contains `touch-browser`
  - set `TOUCH_BROWSER_SERVE_COMMAND="target/debug/touch-browser serve"` if you want to force a specific built binary or wrapper
- verifier hook failure:
  - run the verifier command directly and confirm it returns JSON with an `outcomes` array
  - start with `node scripts/example-verifier.mjs` and replace it only after your own verifier returns the same shape
- supervised interactive action rejected:
  - confirm the allowlisted host
  - use `--headed` only for supervised human recovery on the CLI surface; MCP does not expose headed browsing
  - confirm the required `--ack-risk` or `checkpoint -> approve` step
  - inspect provider hints and the recommended profile in `checkpoint.approvalPanel` and `checkpoint.playbook`
  - use `--sensitive` or the daemon secret store for secret input
  - confirm no other CLI process is holding the same `--session-file`
- update command fails:
  - confirm the current install came from `install.sh` and `~/.touch-browser/install/install-manifest.json` exists
  - confirm outbound HTTPS access to GitHub Releases
  - rerun `touch-browser update --check` to inspect the target asset before install
- uninstall command fails:
  - rerun with `--yes`
  - stop any long-running process that still uses the managed install path
  - if the command path is already gone, run `~/.touch-browser/install/current/uninstall.sh --purge-all --yes` or remove the paths listed below manually

## 6. Notes

- `read-view` and `session-read` emit raw Markdown in direct CLI mode
- `read-view` prefers main-content blocks by default; `--main-only` makes the filter explicit for especially noisy page chrome
- `session-synthesize --format markdown` emits raw Markdown in direct CLI mode
- `serve` and MCP keep returning structured JSON
- MCP package and MCP bridge stay headless and do not expose `engine` or `headed`; challenge, auth, MFA, and similar cases are supervised recovery handoff points
- managed standalone install paths:
  - `~/.touch-browser/install/versions/<bundle-name>`
  - `~/.touch-browser/install/current`
  - `~/.touch-browser/install/install-manifest.json`
  - `~/.touch-browser/install/install-manifest.env`
- `extract` emits four-state claim outcomes: `evidence-supported`, `contradicted`, `insufficient-evidence`, and `needs-more-browsing`
- verifier hooks can adjudicate the final claim verdict, but they still run on top of the same base evidence collector
- `reviewRecommended` and `confidenceBand=review` are the primary escalation signals for verifier-driven operation
- non-sensitive typed values are replayed in the same browser pass right before submit
- sensitive values are replayed only through the direct CLI secret sidecar or the daemon in-memory secret store
- anti-bot, MFA, payment, and other high-risk write actions are handled as supervised flows, not bypass flows
- the default supervised operating procedure is `checkpoint -> approve -> supervised recovery -> refresh`
- headed continuation is an operator-only recovery step on the CLI surface, not an MCP contract
- pilot telemetry defaults to `~/.touch-browser/pilot/telemetry.sqlite` in an installed bundle and `output/pilot/telemetry.sqlite` in a repo checkout
- complete clean removal from the default installed path means deleting:
  - `~/.touch-browser/install`
  - `~/.touch-browser/browser-search`
  - `~/.touch-browser/pilot`
  - `~/.touch-browser/models` when `--purge-all` is used
  - the PATH command symlink such as `~/.local/bin/touch-browser`, `~/bin/touch-browser`, `/usr/local/bin/touch-browser`, or `/opt/homebrew/bin/touch-browser`

## 7. License

This repository is distributed under `MPL-2.0`.

- commercial use is allowed
- if you distribute changes to MPL-covered files, those covered files must stay
  available under `MPL-2.0`
- larger works may include separate files under different terms
- see [LICENSE](../LICENSE) and [LICENSE-POLICY.md](../LICENSE-POLICY.md)
