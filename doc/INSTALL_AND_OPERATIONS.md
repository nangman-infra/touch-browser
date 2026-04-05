# Install And Operations

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `local bootstrap, validation, and day-to-day operation`

## 1. Quick Start

Prerequisites:

- [rustup](https://rustup.rs)
- Node.js 18+
- `pnpm`

Bootstrap:

```bash
bash scripts/bootstrap-local.sh
```

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

## 2. Core Commands

Read a real page:

```bash
cargo run -q -p touch-browser-cli -- read-view https://www.iana.org/help/example-domains
```

For navigation-heavy pages, add `--main-only` to keep the Markdown output centered on the primary content region.

Generate the low-token agent view:

```bash
cargo run -q -p touch-browser-cli -- compact-view https://www.iana.org/help/example-domains
```

Extract evidence with an optional verifier hook:

```bash
cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes." \
  --verifier-command 'node scripts/example-verifier.mjs'
```

Render a multi-page session as Markdown:

```bash
cargo run -q -p touch-browser-cli -- session-synthesize --session-file /tmp/tb-session.json --format markdown
```

Serve daemon:

```bash
cargo run -q -p touch-browser-cli -- serve
```

MCP bridge:

```bash
node scripts/touch-browser-mcp-bridge.mjs
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
  - verify `cargo run -q -p touch-browser-cli -- serve` works on its own
- verifier hook failure:
  - run the verifier command directly and confirm it returns JSON with an `outcomes` array
  - start with `node scripts/example-verifier.mjs` and replace it only after your own verifier returns the same shape
- supervised interactive action rejected:
  - confirm the allowlisted host
  - use `--headed` for live non-fixture targets
  - confirm the required `--ack-risk` or `checkpoint -> approve` step
  - inspect provider hints and the recommended profile in `checkpoint.approvalPanel` and `checkpoint.playbook`
  - use `--sensitive` or the daemon secret store for secret input
  - confirm no other CLI process is holding the same `--session-file`

## 6. Notes

- `read-view` and `session-read` emit raw Markdown in direct CLI mode
- `read-view` prefers main-content blocks by default; `--main-only` makes the filter explicit for especially noisy page chrome
- `session-synthesize --format markdown` emits raw Markdown in direct CLI mode
- `serve` and MCP keep returning structured JSON
- `extract` emits four-state claim outcomes: `evidence-supported`, `contradicted`, `insufficient-evidence`, and `needs-more-browsing`
- verifier hooks can adjudicate the final claim verdict, but they still run on top of the same base evidence collector
- non-sensitive typed values are replayed in the same browser pass right before submit
- sensitive values are replayed only through the direct CLI secret sidecar or the daemon in-memory secret store
- anti-bot, MFA, payment, and other high-risk write actions are handled as supervised flows, not bypass flows
- the default supervised operating procedure is `checkpoint -> approve -> headed continuation -> refresh`
- pilot telemetry is stored in `output/pilot/telemetry.sqlite` by default

## 7. License

This repository is distributed under `BUSL-1.1`.

- allowed without a commercial license: self-hosted evaluation, development, testing
- production, hosted, or commercial operation requires a commercial license
- see [LICENSE](../LICENSE) and [LICENSE-POLICY.md](../LICENSE-POLICY.md)
