# Install And Operations

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `local bootstrap, validation, and day-to-day operation`

## 1. Quick Start

Requirements:

- Rust toolchain with `rustfmt` and `clippy`
- `pnpm`
- Node.js LTS

Bootstrap:

```bash
bash scripts/bootstrap-local.sh
```

Manual setup:

```bash
rustup component add rustfmt clippy
pnpm install
pnpm exec playwright install chromium
cargo build -q --workspace
pnpm typecheck
pnpm test
```

## 2. Core Commands

CLI:

```bash
cargo run -q -p touch-browser-cli -- compact-view https://www.iana.org/help/example-domains
```

Supervised interactive example:

```bash
cargo run -q -p touch-browser-cli -- open https://github.com/login --browser --headed --allow-domain github.com --session-file /tmp/tb-login.json
cargo run -q -p touch-browser-cli -- checkpoint --session-file /tmp/tb-login.json
cargo run -q -p touch-browser-cli -- session-profile --session-file /tmp/tb-login.json
cargo run -q -p touch-browser-cli -- approve --session-file /tmp/tb-login.json --risk auth
cargo run -q -p touch-browser-cli -- type --session-file /tmp/tb-login.json --ref <user-ref> --value <user> --headed
cargo run -q -p touch-browser-cli -- type --session-file /tmp/tb-login.json --ref <password-ref> --value <secret> --headed --sensitive
cargo run -q -p touch-browser-cli -- submit --session-file /tmp/tb-login.json --ref <form-ref> --headed
cargo run -q -p touch-browser-cli -- refresh --session-file /tmp/tb-login.json --headed
cargo run -q -p touch-browser-cli -- telemetry-summary
```

Serve daemon:

```bash
cargo run -q -p touch-browser-cli -- serve
```

MCP bridge:

```bash
node scripts/touch-browser-mcp-bridge.mjs
```

Public proof runs:

```bash
pnpm run fixtures:public-web
pnpm run pilot:public-reference-workflow
pnpm run pilot:real-user-research
```

Self-hosted pilot package:

```bash
docker build -f deploy/Dockerfile -t touch-browser:pilot .
docker compose -f deploy/docker-compose.pilot.yml up --build
pnpm run pilot:healthcheck
```

Related operations package:

- [OPERATIONS_SECURITY_PACKAGE_SPEC.md](OPERATIONS_SECURITY_PACKAGE_SPEC.md)
- environment example: [deploy/touch-browser.env.example](../deploy/touch-browser.env.example)

## 3. Operational Checks

- `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm typecheck`
- `pnpm test`
- `pnpm run pilot:healthcheck`
- optionally `pnpm run fixtures:public-web`
- optionally `pnpm run pilot:public-reference-workflow`
- optionally `pnpm run fixtures:safety`

## 4. Troubleshooting

- browser launch failure:
  - run `pnpm exec playwright install chromium`
- Rust command not found:
  - confirm `rustup` and `cargo` are on the shell `PATH`
- public benchmark failure:
  - check network access and remote site availability
- MCP bridge failure:
  - verify `cargo run -q -p touch-browser-cli -- serve` works on its own
- supervised interactive action rejected:
  - confirm the allowlisted host
  - use `--headed` for live non-fixture targets
  - confirm the required `--ack-risk` or `checkpoint -> approve` step
  - inspect provider hints and the recommended profile in `checkpoint.approvalPanel` and `checkpoint.playbook`
  - use `--sensitive` or the daemon secret store for secret input
  - confirm no other CLI process is holding the same `--session-file`
- pin a supervised auth or write profile explicitly:
  - `touch-browser set-profile --session-file <path> --profile interactive-supervised-auth|interactive-supervised-write`
- split pilot telemetry into a separate database:
  - `TOUCH_BROWSER_TELEMETRY_DB=/tmp/tb-pilot.sqlite`
  - `TOUCH_BROWSER_TELEMETRY_SURFACE=cli|serve|mcp`

## 5. Notes

- credential-like typing and form submit are only supported inside allowlisted interactive sessions
- non-sensitive typed values are replayed in the same browser pass right before submit
- sensitive values are replayed only through the direct CLI secret sidecar or the daemon in-memory secret store
- anti-bot, MFA, payment, and other high-risk write actions are handled as supervised flows, not bypass flows
- the default supervised operating procedure is `checkpoint -> approve -> headed continuation -> refresh`
- provider-specific auth and challenge guidance is exposed through `checkpoint.playbook`
- pilot telemetry is stored in `output/pilot/telemetry.sqlite` by default and can be queried directly with the summary and recent commands
