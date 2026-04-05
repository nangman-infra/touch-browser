# Install And Operations

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `local bootstrap, validate, and operate touch-browser`

## 1. 빠른 시작

필수:

- Rust toolchain with `rustfmt`, `clippy`
- `pnpm`
- Node.js LTS

로컬 부트스트랩:

```bash
bash scripts/bootstrap-local.sh
```

수동 순서:

```bash
rustup component add rustfmt clippy
pnpm install
pnpm exec playwright install chromium
cargo build -q --workspace
pnpm typecheck
pnpm test
```

## 2. 핵심 실행

CLI:

```bash
cargo run -q -p touch-browser-cli -- compact-view fixture://research/static-docs/getting-started
```

supervised interactive 예시:

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

serve daemon:

```bash
cargo run -q -p touch-browser-cli -- serve
```

MCP bridge:

```bash
node scripts/touch-browser-mcp-bridge.mjs
```

public benchmark:

```bash
pnpm run fixtures:public-web
pnpm run pilot:public-reference-workflow
pnpm run pilot:real-user-research
```

self-hosted pilot package:

```bash
docker build -f deploy/Dockerfile -t touch-browser:pilot .
docker compose -f deploy/docker-compose.pilot.yml up --build
pnpm run pilot:healthcheck
```

관련 운영 패키지:

- [OPERATIONS_SECURITY_PACKAGE_SPEC.md](OPERATIONS_SECURITY_PACKAGE_SPEC.md)
- env example: [deploy/touch-browser.env.example](../deploy/touch-browser.env.example)

## 3. 운영 체크

- `cargo clippy --workspace --all-targets -- -D warnings`
- `pnpm typecheck`
- `pnpm test`
- `pnpm run pilot:healthcheck`
- 필요 시 `pnpm run fixtures:public-web`
- 필요 시 `pnpm run pilot:public-reference-workflow`
- 필요 시 `pnpm run fixtures:safety`

## 4. 장애 확인

- 브라우저 실행 실패:
  - `pnpm exec playwright install chromium`
- Rust command not found:
  - shell PATH에서 `rustup`/`cargo` 확인
- 공개 웹 벤치마크 실패:
  - 네트워크 상태 또는 원격 사이트 가용성 확인
- MCP bridge 실패:
  - `cargo run -q -p touch-browser-cli -- serve` 단독 동작 확인
- supervised interactive action이 거절될 때:
  - allowlist host 확인
  - live non-fixture면 `--headed` 사용 여부 확인
  - 필요한 `--ack-risk` 또는 `checkpoint -> approve`가 적용됐는지 확인
  - `checkpoint.approvalPanel`과 `checkpoint.playbook`의 provider hint / recommended profile 확인
  - 민감값이면 `--sensitive` 또는 daemon secret store 사용 여부 확인
  - 같은 `--session-file`을 다른 CLI가 동시에 사용 중인지 확인
- supervised auth/write profile을 명시적으로 고정하고 싶을 때:
  - `touch-browser set-profile --session-file <path> --profile interactive-supervised-auth|interactive-supervised-write`
- pilot telemetry를 분리 저장하고 싶을 때:
  - `TOUCH_BROWSER_TELEMETRY_DB=/tmp/tb-pilot.sqlite`
  - `TOUCH_BROWSER_TELEMETRY_SURFACE=cli|serve|mcp`

## 5. 현재 한계

- credential-like type과 form submit은 allowlisted interactive session 안에서 제한적으로 지원합니다.
- non-sensitive typed value는 submit 직전에 같은 browser pass 안에서 재적용됩니다.
- sensitive typed value는 direct CLI에서는 session-file 옆 secret sidecar, daemon에서는 in-memory secret store로만 재적용됩니다.
- anti-bot, MFA, 결제/고위험 write action은 현재 `우회`가 아니라 `감독형 supervised flow` 범위입니다.
- CAPTCHA/MFA/auth/high-risk write는 `checkpoint -> approve -> headed continuation -> refresh` 흐름을 기본 운영 절차로 봅니다.
- provider별 auth/challenge는 `checkpoint.playbook`으로 GitHub/Google/Microsoft/Okta/Auth0/generic 흐름을 안내합니다.
- pilot telemetry는 기본적으로 `output/pilot/telemetry.sqlite`에 누적되며, summary/recent 명령으로 바로 확인할 수 있습니다.
- live provider별 인증 성공 보장, CAPTCHA 해결, 실제 결제 완료 보장은 아직 제품 범위 밖입니다.
