# CLI Surface Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `touch-browser binary commands and JSON outputs`

## 1. 목적

이 문서는 현재 `touch-browser` CLI의 실제 명령 표면을 고정합니다.

현재 범위:

- stable research surface
- experimental supervised surface
- fixture 대상 read-only browsing
- live URL open/extract/policy
- Playwright browser-backed open/extract/policy
- persisted browser session commands
- allowlisted interactive browser session commands
- compact snapshot commands
- session synthesis command
- browser replay command
- stdio JSON-RPC serve mode
- long-lived daemon session registry
- multi-tab daemon orchestration
- replay / memory summary utilities
- JSON-only stdout contract

## 2. 현재 명령

stable research surface:

| Command | 설명 |
| --- | --- |
| `touch-browser open <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | target을 열고 structured snapshot을 만듭니다. |
| `touch-browser snapshot <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | open 결과에서 full snapshot payload를 직접 확인합니다. |
| `touch-browser compact-view <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | AI 입력용 compact semantic text와 `refIndex`를 반환합니다. |
| `touch-browser extract <target> --claim <statement> ... [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | claim별 supported/unsupported evidence와 citation metadata를 반환합니다. |
| `touch-browser policy <target> [--browser] [--headed] [--budget <tokens>] [--allow-domain <host> ...]` | allow/review/block policy report를 반환합니다. |
| `touch-browser session-snapshot --session-file <path>` | persisted browser session의 latest snapshot을 조회합니다. |
| `touch-browser session-compact --session-file <path>` | persisted browser session의 compact semantic view를 반환합니다. |
| `touch-browser session-extract --session-file <path> --claim <statement> ...` | persisted browser session에 대해 evidence extraction을 수행합니다. |
| `touch-browser session-synthesize --session-file <path> [--note-limit <count>]` | multi-page session을 structured notes, claims, citations로 종합합니다. |
| `touch-browser follow --session-file <path> --ref <stable-ref> [--headed]` | stable ref를 따라 persisted browser session을 진행합니다. |
| `touch-browser paginate --session-file <path> --direction next|prev [--headed]` | persisted browser session에서 pagination action을 수행합니다. |
| `touch-browser expand --session-file <path> --ref <stable-ref> [--headed]` | persisted browser session에서 expandable block을 엽니다. |
| `touch-browser browser-replay --session-file <path>` | persisted browser session의 action trace를 replay 관점에서 재구성합니다. |
| `touch-browser session-close --session-file <path>` | persisted browser session과 browser context를 정리합니다. |
| `touch-browser telemetry-summary` | pilot telemetry aggregate summary를 반환합니다. |
| `touch-browser telemetry-recent [--limit <count>]` | 최근 telemetry event 목록을 반환합니다. |
| `touch-browser replay <scenario-name>` | recorded scenario transcript를 다시 재생합니다. |
| `touch-browser memory-summary [--steps <even-number>]` | long-session memory compaction 요약을 생성합니다. |
| `touch-browser serve` | stdio JSON-RPC daemon을 실행합니다. |

experimental supervised surface:

| Command | 설명 |
| --- | --- |
| `touch-browser checkpoint --session-file <path>` | 현재 supervised browser state의 risk, provider hint, approval guidance를 반환합니다. |
| `touch-browser session-policy --session-file <path>` | persisted browser session의 policy report를 직접 조회합니다. |
| `touch-browser session-profile --session-file <path>` | 현재 supervised policy profile을 조회합니다. |
| `touch-browser set-profile --session-file <path> --profile research-read-only|research-restricted|interactive-review|interactive-supervised-auth|interactive-supervised-write` | supervised policy profile을 변경합니다. |
| `touch-browser approve --session-file <path> --risk challenge|mfa|auth|high-risk-write [--risk ...]` | required risk acknowledgement를 session에 기록합니다. |
| `touch-browser click --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]` | supervised session 안에서 interactive click을 실행합니다. |
| `touch-browser type --session-file <path> --ref <stable-ref> --value <text> [--headed] [--sensitive] [--ack-risk challenge|mfa|auth|high-risk-write ...]` | supervised session 안에서 값을 입력합니다. |
| `touch-browser submit --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]` | supervised session 안에서 form/control submit을 실행합니다. |
| `touch-browser refresh --session-file <path> [--headed]` | supervised session을 새 상태로 다시 컴파일합니다. |

이 구분은 제품 경계용 분류이며, 구현 자체는 같은 바이너리 안에 공존합니다.

## 3. 현재 경로 분리

- fixture target은 `ReadOnlyActionVm`을 통해 실행
- live target은 `ReadOnlyRuntime + AcquisitionEngine + PolicyKernel` handoff를 통해 실행
- browser target은 `Playwright stdio adapter -> ObservationCompiler -> ReadOnlyRuntime.open_snapshot -> Policy/Evidence` 경로로 실행
- persisted browser session은 `session-file JSON -> ReadOnlySession + persisted browser state/browser context dir/browser trace/requested budget restore -> stable ref -> adapter hint(targetTagName/targetHref/targetDomPathHint/targetOrdinalHint) -> Playwright action -> Runtime append -> session-file save` 경로로 실행
- browser interactive action은 현재 `allowlist + policy preflight + sensitive field opt-in`을 통과한 경우에만 type/click/submit을 허용
- supervised browser action은 추가로 `ack-risk`와 live site일 때 `--headed` 조건을 통과해야 계속 진행됩니다.
- `checkpoint`는 현재 live page에 대해 provider hint, required ack risk, active/recommended policy profile, approval panel, provider-specific playbook, 후보 control을 반환합니다.
- `approve`는 session file에 승인된 risk를 저장해 이후 interactive 명령에서 같은 `--ack-risk`를 반복하지 않게 하고, supervised auth/write profile로 승격할 수 있습니다.
- `session-profile` / `set-profile`은 persisted browser session의 정책 프로필을 직접 조회/변경합니다.
- `telemetry-summary` / `telemetry-recent`는 pilot telemetry SQLite를 직접 조회합니다.
- 같은 `--session-file`을 공유하는 browser action은 persistent context lock으로 직렬화됩니다.

현재 live/browser에서 하는 것:

- `open`
- `extract`
- `policy`
- `follow`
- `click`
- `type`
- `submit`
- `paginate`
- `expand`

serve daemon에서 하는 것:

- `runtime.session.create`
- `runtime.session.open`
- `runtime.session.snapshot`
- `runtime.session.compactView`
- `runtime.session.extract`
- `runtime.session.policy`
- `runtime.session.synthesize`
- `runtime.session.follow`
- `runtime.session.click`
- `runtime.session.type`
- `runtime.session.typeSecret`
- `runtime.session.submit`
- `runtime.session.refresh`
- `runtime.session.checkpoint`
- `runtime.session.approve`
- `runtime.session.profile.get`
- `runtime.session.profile.set`
- `runtime.session.secret.store`
- `runtime.session.secret.clear`
- `runtime.telemetry.summary`
- `runtime.telemetry.recent`
- `runtime.session.paginate`
- `runtime.session.expand`
- `runtime.session.replay`
- `runtime.session.close`
- `runtime.tab.open`
- `runtime.tab.list`
- `runtime.tab.select`
- `runtime.tab.close`

아직 하지 않는 것:

- live `follow`
- live dynamic actions
- native shared browser context multi-tab

## 4. 출력 원칙

- stdout은 항상 JSON
- human-friendly prose 출력 없음
- 실패는 stderr + non-zero exit code

주요 출력 형태:

- `open` / `snapshot` -> `ActionResult`
- `compact-view` / `session-compact` -> compact snapshot payload + `refIndex`
- `extract` -> `open` + `extract` + `sessionState`
- `policy` -> `policy` + `sessionState`
- `session-synthesize` -> `report` + `sessionState` + `sessionFile`
- `browser-replay` -> `replayedActions` + `compactText` + `sessionState`
- `replay` -> `sessionState` + `replayTranscript` + counts
- `memory-summary` -> `requestedActions` + `actionCount` + `sessionState` + `memorySummary`
- `serve` -> line-delimited JSON-RPC responses only

interactive action 계열(`click`, `type`, `submit`, `refresh`)은 현재:

- `action`
- `policy`
- `sessionState`
- `result`

형태를 함께 반환합니다. `result`는 상위 호환 alias이고, 실제 실행 결과 해석은 `action` 필드를 기준으로 합니다.

`checkpoint`는 `checkpoint`, `policy`, `result`, `sessionState`를 반환하고, `checkpoint.approvalPanel` / `checkpoint.playbook` / `checkpoint.activePolicyProfile` / `checkpoint.recommendedPolicyProfile`까지 포함합니다. `approve`는 `approvedRisks`, `policyProfile`, `result`, `sessionState`, `sessionFile`을 반환합니다. `telemetry-summary`는 aggregate summary를, `telemetry-recent`는 최근 event 목록을 반환합니다.

## 5. 현재 검증 범위

Rust 테스트:

- fixture open CLI test
- hostile policy CLI test
- browser-backed fixture open CLI test
- browser-backed extract CLI test
- browser-backed hostile policy CLI test
- browser session snapshot persistence test
- browser session paginate test
- browser session double-paginate DOM persistence test
- browser session follow + session-extract test
- browser session duplicate-follow stable ref ordinal test
- browser session requested budget persistence test
- browser session expand + session-extract test
- browser session interactive type test
- browser session sensitive input rejection test
- browser session interactive click test
- browser session interactive submit test
- browser session supervised MFA submit test
- browser session supervised high-risk submit test
- browser session refresh test
- browser session checkpoint / approve persistence test
- session profile parse/set tests
- telemetry store / summary tests
- compact-view CLI test
- session-compact CLI test
- browser-replay CLI test
- session-close browser context cleanup test
- replay CLI test
- 50-action memory CLI test

직접 실행 검증:

- fixture `open --browser`
- pricing `extract --browser`
- hostile `policy --browser`
- browser session `open -> follow -> session-extract -> session-close`
- browser session `open -> type -> click -> session-close`
- browser session `open -> type -> submit -> session-close`
- browser session `open -> sensitive type -> supervised submit -> refresh -> session-close`
- browser session `open -> checkpoint -> approve -> supervised auth type/submit -> refresh -> session-close`
- real-site `open -> checkpoint -> approve -> type/typeSecret -> submit -> refresh` GitHub auth smoke
- browser session `open -> paginate -> session-close`
- browser session `open -> expand -> session-extract -> session-close`
- browser session `open -> session-compact -> session-synthesize -> browser-replay -> session-close`
- stdio JSON-RPC `serve -> runtime.status -> runtime.open`
- stdio JSON-RPC `serve -> runtime.session.create -> runtime.session.open -> runtime.tab.open -> runtime.session.synthesize -> runtime.session.close`
- stdio JSON-RPC `serve -> runtime.session.secret.store -> runtime.session.typeSecret -> runtime.session.submit -> runtime.session.refresh`
- stdio JSON-RPC `serve -> runtime.session.profile.get|set -> runtime.telemetry.summary|recent`
- MCP bridge `initialize -> tools/list -> tools/call(tb_status)`

## 6. 현재 한계

- compact text는 token 효율을 위해 `block id`를 제거하고 kind + 짧은 digest만 유지하며, action용 stable ref는 `refIndex`로 분리합니다.
- serve daemon은 장기 프로세스 안에 session registry와 tab registry를 유지하지만, 각 tab은 별도 persistent browser context/session-file을 사용합니다.
- browser-backed `follow`는 persisted session 경로에서만 지원하며 live multi-step replay는 아직 없음
- `--budget`은 live/browser open 계열에서 observation requested token budget을 제어하고, browser session 안에서 follow/paginate/expand 재컴파일에도 그대로 유지됩니다.
- interactive action은 아직 allowlisted browser session 안에서만 지원합니다.
- sensitive credential-like input은 `--sensitive` 없이 type할 수 없습니다.
- submit은 form stable ref 또는 submit control stable ref를 기준으로 동작합니다.
- submit은 같은 세션에서 입력된 non-sensitive type 값을 같은 browser pass 안에 다시 적용한 뒤 form/control submit을 수행합니다.
- sensitive value는 출력과 replay에서 계속 redacted됩니다.
- direct CLI는 session-file 옆 secret sidecar를 사용해 같은 세션 submit/replay에만 sensitive value를 재적용합니다.
- serve daemon은 session memory의 secret store를 사용하고 `runtime.session.typeSecret` / `runtime.session.secret.store`로 값을 주입합니다.
- CAPTCHA, MFA, sensitive auth, high-risk write는 우회 대상이 아니라 감독형 `review-gated` 흐름입니다.
- interactive action은 해당 signal이 감지되면 필요한 `--ack-risk` 없이 실행되지 않습니다.
- `checkpoint -> approve` 경로를 쓰면 같은 session 안에서는 필요한 risk 승인을 반복해서 넘기지 않아도 됩니다.
- supervised flow는 provider-specific playbook과 approval panel을 반환해 GitHub/Google/Auth0/Okta/Microsoft 계열 auth와 bot challenge를 구조적으로 안내합니다.
- policy profile은 현재 `research-read-only`, `research-restricted`, `interactive-review`, `interactive-supervised-auth`, `interactive-supervised-write`를 지원합니다.
- pilot telemetry는 기본적으로 `output/pilot/telemetry.sqlite`에 기록되며 `TOUCH_BROWSER_TELEMETRY_DB` / `TOUCH_BROWSER_TELEMETRY_SURFACE`로 override할 수 있습니다.
- live non-fixture site에서 supervised interactive action을 계속하려면 현재 `--headed`가 필요합니다.
- persistent context는 cross-process lock으로 보호되며, 다른 명령이 같은 session을 이미 사용 중이면 잠시 대기하거나 busy error를 반환합니다.
- allowlist는 현재 domain boundary와 ref preflight 차단까지 지원합니다.
- subcommand별 schema 분리 없음
- stdout contract는 안정적이지만 아직 JSON Schema로 개별 명령마다 고정되지 않음
