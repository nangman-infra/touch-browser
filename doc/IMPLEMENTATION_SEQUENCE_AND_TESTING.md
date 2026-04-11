# Implementation Sequence And Testing

- Status: `Active`
- Last Updated: `2026-04-11`
- Scope: `build order and test strategy`

## 1. 목적

이 문서는 `무엇부터 만들 것인가`와 `어떻게 검증할 것인가`를 고정합니다.

핵심 원칙:

- 구현 순서는 기술 의존성만이 아니라 `초기 고객 wedge`와 `Strong Go 게이트`를 기준으로 정합니다.
- 테스트는 나중에 붙이는 보조물이 아니라, 각 단계의 종료 조건입니다.

## 1.1 현재 실행 상태

- Phase 0 완료: schema baseline 추가, contract validation tests 통과
- Phase 1 완료: fixture 30개, expected snapshot/evidence baseline, fixture metadata validation tests 통과
- Rust workspace scaffold 검증 완료: `cargo check`, `cargo test` 통과
- Phase 2 완료: Observation Compiler v0와 golden snapshot tests 통과
- Phase 3 완료: Evidence/Citation v0 baseline과 golden evidence tests 통과, 30개 corpus 정량 baseline 확보
- Phase 4 진행 중: Read-only Session / Replay v0와 deterministic transcript baseline, live multi-page session synthesis baseline 추가
- Phase 5 완료: Acquisition Engine v0 local/live baseline과 runtime handoff tests 통과
- Phase 6 진행 중: 20-step / 50-step memory summary baseline과 memory rollup tests 통과, local live session note rollup 추가
- Phase 7 진행 중: Policy Kernel v0 hostile snapshot block/review seed tests 통과, allowlist/preflight/policy report contract/eval 연결 완료
- Phase 8 진행 중: Action VM v0 policy-integrated read-only typed action tests 통과, usable CLI surface와 persisted browser session CLI 추가
- Phase 9 완료: Playwright browser-backed snapshot/dynamic action baseline과 CLI sidecar handoff tests 통과
- G1 baseline 추가: observation metrics report와 regression eval 통과
- G1 browser-backed baseline 추가: Playwright representative subset metrics report와 regression eval 통과
- G1 live baseline 추가: local live observation metrics report와 regression eval 통과
- WP-05 baseline 추가: citation metrics report와 regression eval 통과
- G3 proxy baseline 추가: action-surface regression report와 regression eval 통과
- G5/G6 internal/live baseline 추가: latency-cost, customer-fit-economics, serve smoke, serve daemon smoke, public-web benchmark, MCP smoke 통과

## 1.2 Release-first Lifecycle Test Ownership

이번 release-first 정리에서 새로 생긴 책임은 아래 세 가지입니다.

- Google search trust profile을 엔진별 persistent profile로 유지한다
- `touch-browser update`가 managed install을 안전하게 갱신한다
- `touch-browser uninstall`이 destructive semantics를 명확히 지키며 clean purge를 수행한다

중요한 전제:

- 현재 SonarQube coverage는 Playwright adapter TypeScript LCOV를 기준으로 반영됩니다.
- Rust CLI lifecycle 변경은 Sonar 퍼센트보다 `Rust regression tests + local quality gate + 필요 시 실사용 proof`로 방어합니다.
- 따라서 Rust 쪽 테스트를 줄일 때는 coverage 숫자가 아니라 `회귀 탐지 책임이 비어 있지 않은지`를 먼저 봐야 합니다.

### CI에 반드시 남겨야 하는 테스트

- search profile path / state metadata
  - `core/crates/cli/src/application/search_support.rs`
  - 기본 profile 경로, profile-state metadata, challenge -> manual recovery 상태 전이를 검증
- Google challenge policy / fallback semantics
  - `core/crates/cli/src/application/research_commands.rs`
  - explicit Google 요청은 Brave fallback으로 새지 않고, challenge hint가 같은 saved profile 재사용을 가리키는지 검증
- update / uninstall command grammar
  - `core/crates/cli/src/interface/cli_tests_parsing.rs`
  - `update --check`, `update --version`, `uninstall --purge-all --yes` 같은 파싱 경계를 검증
- managed install / purge semantics
  - `core/crates/cli/src/infrastructure/installation.rs`
  - managed install 경로, release asset 선택, version normalization, uninstall keep/purge semantics를 검증
- standalone lifecycle smoke
  - `evals/tests/runtime/gate/standalone-lifecycle.test.ts`
  - fake standalone bundle + local release server로 `install -> update -> uninstall --purge-all` 전체를 실제 installed command 경로로 검증
- uninstall telemetry guard
  - `core/crates/cli/src/interface/cli_entry.rs`
  - uninstall 후 telemetry DB가 다시 생기지 않도록 lifecycle logging skip을 검증
- repository-level gate
  - `pnpm run quality:ci`
  - `pnpm run architecture:check`

### 수동 실사용 proof로만 남길 것

- 실제 Google challenge page를 headed mode로 열고 사람이 한 번 풀어 같은 profile이 재사용되는지 확인
- 실제 release asset과 실제 bundled runtime으로 `install -> update -> uninstall --purge-all` 전체 lifecycle을 한 번 더 확인
- 실제 GitHub Release publish 이후 tag asset과 same-SHA GitHub / Sonar convergence 확인

### 매 변경마다 다시 할 필요가 없는 것

- parser / 문서 변경만 있었는데 live Google challenge recovery를 다시 수행하는 것
- managed install 코드가 안 바뀌었는데 실제 release asset lifecycle proof를 반복하는 것
- release tag를 새로 발행하지 않았는데 실제 GitHub Release publish를 다시 수행하는 것

### Test Pruning Rule

- 어떤 테스트든 제거 전에 `동일하거나 더 강한 검증이 더 저렴한 계층에 이미 존재한다`는 근거가 있어야 합니다.
- 아래 항목의 유일한 방어 테스트는 제거하지 않습니다.
  - default search profile 경로
  - challenge hint 정책
  - destructive uninstall confirmation / purge semantics
  - uninstall telemetry skip
- 수동 proof를 근거로 CI 테스트를 지우지 않습니다.
- 먼저 manual proof를 unit 또는 integration test로 끌어내린 뒤에만 기존 회귀 테스트 축소를 검토합니다.

## 2. 가장 중요한 판단

초기 고객군이 `Research Agent Platform Teams`로 고정되었기 때문에, 첫 구현 순서는 `동적 브라우저 자동화`가 아닙니다.

먼저 만들어야 할 것은 아래입니다.

- AI-native observation contract
- reproducible fixture corpus
- citation/evidence pipeline
- replayable read-only research loop

즉 초기 제품의 첫 완성 루프는 다음입니다.

`URL or fixture input -> semantic snapshot -> evidence extraction -> citation -> replay -> evaluation`

아직 첫 번째 루프에 포함하지 않는 것:

- 로그인 자동화
- 폼 제출
- 구매/예약/설정 변경
- anti-bot/stealth/CAPTCHA
- screenshot-first fallback 중심 설계

## 3. 구현 순서

## 3.1 Phase 0. Contract Baseline

목표:

- 모든 경계를 schema로 먼저 고정합니다.

구현 항목:

- `snapshot-block.schema.json`
- `evidence-block.schema.json`
- `action-command.schema.json`
- `session-state.schema.json`
- `replay-transcript.schema.json`
- `json-rpc-request.schema.json`
- `json-rpc-response.schema.json`

왜 먼저 하는가:

- Rust core와 TS sidecar가 어긋나는 가장 빠른 경로가 contract drift입니다.
- contract가 없으면 fixture도, grader도, replay도 전부 흔들립니다.

완료 조건:

- schema 파일이 존재
- manifest 생성 가능
- TS validator wiring 방향 확정

필수 테스트:

- schema validity test
- example payload validation test
- invalid payload rejection test

## 3.2 Phase 1. Fixture Corpus And Graders

목표:

- 외부 웹의 변동성과 분리된 기준 데이터를 만듭니다.

구현 항목:

- static docs fixtures
- structured table/list fixtures
- navigation fixtures
- citation-heavy fixtures
- hostile fixtures
- 기대 snapshot/evidence/grading rules

왜 여기서 하는가:

- observation compiler를 라이브 웹에 먼저 붙이면 “좋아진 것처럼 보이는 착시”가 생깁니다.
- 먼저 통제된 fixture에서 품질 기준을 세워야 합니다.

완료 조건:

- 최소 30개 fixture
- fixture별 grading metadata 존재
- hostile fixture 최소 5개

필수 테스트:

- fixture loading test
- grader rule validity test
- fixture metadata completeness test

현재 상태:

- research fixture corpus 30개 구성 완료
- static-docs / navigation / citation-heavy / hostile 카테고리 baseline 구성 완료
- fixture별 expected snapshot baseline 생성 완료
- fixture별 expected evidence baseline 생성 완료
- hostile fixture 7개와 claimChecks grading metadata 연결 완료

## 3.3 Phase 2. Observation Compiler v0

목표:

- 정적 HTML/fixture 입력을 semantic snapshot으로 변환합니다.

구현 항목:

- DOM normalization
- visible text extraction
- heading/list/table/link/form semantics
- stable ref generation
- token budget ranking
- boilerplate suppression

왜 이 단계가 핵심인가:

- G1 Observation format은 이 프로젝트의 첫 증명 조건입니다.
- 여기가 약하면 acquisition이나 dynamic browsing을 붙여도 제품 의미가 없습니다.

완료 조건:

- fixture -> snapshot JSON 생성
- snapshot determinism 확보
- stable ref spec 문서화
- golden snapshot baseline 생성

필수 테스트:

- golden snapshot test
- stable ref determinism test
- token budget truncation test
- property test for ref stability and block ordering invariants

## 3.4 Phase 3. Evidence And Citation v0

목표:

- snapshot에서 근거 연결 가능한 추출 구조를 만듭니다.

구현 항목:

- claim-evidence linking
- citation payload
- unsupported claim detection
- source offsets

왜 이 단계가 observation 다음인가:

- 초기 wedge의 구매 이유는 브라우징 자체가 아니라 `evidence and citations`입니다.

완료 조건:

- extract 결과가 항상 support block을 가짐
- unsupported claim이 별도 분리됨

필수 테스트:

- citation precision fixture test
- unsupported claim detection test
- evidence support completeness test

현재 상태:

- fixture metadata에 `claimChecks`와 `expectedEvidencePath` 추가
- deterministic evidence report baseline 생성 완료
- Rust evidence golden report tests 통과
- TS evidence schema/eval tests 통과
- generated citation metrics scenario 추가 완료
- 30개 fixture corpus 기준 citation precision / recall baseline `1.00 / 1.00` 확인

## 3.5 Phase 4. Read-only Session And Replay v0

목표:

- 안전한 읽기 중심 research loop를 먼저 완성합니다.

구현 항목:

- session state
- open/read/follow/extract/diff/compact
- replay transcript
- deterministic session log

왜 여기서 read-only를 먼저 하는가:

- research wedge에 맞고
- 위험한 write-action 없이도 초기 가치가 충분하며
- replay가 early debug primitive가 됩니다.

완료 조건:

- fixture 세션 replay 가능
- session delta 비교 가능

필수 테스트:

- transcript round-trip test
- replay determinism test
- diff correctness test

현재 상태:

- fixture-backed read-only runtime 구현 완료
- transcript round-trip Rust test 통과
- replay determinism Rust test 통과
- session-state / replay-transcript TS baseline tests 통과

## 3.6 Phase 5. Acquisition Engine v0

목표:

- live web fetch를 붙이되 research-safe 범위에 한정합니다.

구현 항목:

- HTTP fetch
- redirect handling
- cache
- content-type gate
- robots policy
- canonical URL normalization

왜 observation보다 뒤인가:

- observation 품질을 먼저 통제된 환경에서 고정해야 acquisition이 의미를 가집니다.

완료 조건:

- 실제 URL에서 HTML 수집 가능
- fetch metadata 기록 가능

필수 테스트:

- redirect handling test
- robots policy test
- cache hit/miss test
- content-type gate test

현재 상태:

- fixture 및 live HTTP fetch 구현 완료
- canonical cache key와 final URL alias cache 구현 완료
- `acquisition-record` transcript payload 추가 완료
- local HTTP acquisition -> runtime live open integration test 통과

## 3.7 Phase 6. Memory And Compaction v0

목표:

- 긴 research 세션에서 토큰 폭증 없이 상태를 유지합니다.

구현 항목:

- working memory
- session memory
- snapshot delta
- compaction policy
- task-aware note synthesis

왜 acquisition 뒤인가:

- compaction은 실제 세션 흐름이 있어야 품질을 검증할 수 있습니다.

완료 조건:

- 20-step 이상 세션 유지
- compact 전후 비교 가능

필수 테스트:

- compaction invariant test
- memory retention regression test
- token reduction measurement test

현재 상태:

- `MemoryTurn` / `MemorySessionSummary` 구현 완료
- generated `memory-20-step` scenario 추가 완료
- generated `memory-50-step` scenario 추가 완료
- Rust 20-action memory baseline test 통과
- TS 20-step / 50-step summary eval tests 추가 완료
- local live `session-synthesize` scenario와 note retention eval 추가 완료

## 3.8 Phase 7. Policy Kernel v0

목표:

- 페이지 내용과 시스템 규칙을 구조적으로 분리합니다.

구현 항목:

- trust zone
- risk class
- read-only research policy
- domain allowlist
- hostile content handling

왜 dynamic actions보다 먼저 하는가:

- research wedge라도 hostile content risk는 즉시 존재합니다.
- 안전 경계를 늦게 붙이면 나중에 전면 재설계가 필요해집니다.

완료 조건:

- hostile fixture 차단
- page content와 policy channel 분리

필수 테스트:

- prompt injection fixture test
- policy block reason test
- allowlist enforcement test

현재 상태:

- snapshot 기반 `allow/review/block` policy seed 구현 완료
- hostile source escalation, suspicious CTA, external actionable block tests 통과
- `policy-report.schema.json` boundary 추가 완료
- generated hostile/static policy regression eval 추가 완료
- domain allowlist, session-file allowlist persistence, follow/expand preflight block 구현 완료
- transcript boundary는 아직 미구현

## 3.9 Phase 8. Action VM v0

목표:

- 자유 문자열 명령이 아니라 typed action 실행 경계를 만듭니다.

구현 항목:

- read-only typed action execution
- action result
- failure taxonomy
- interactive action rejection

완료 조건:

- read-only action 결과가 분류 가능
- action result contract가 존재

필수 테스트:

- success action result test
- blocked interactive action test
- failure classification test

현재 상태:

- `action-result.schema.json` 추가 완료
- read-only action VM seed 구현 완료
- action result에 `policy` report 결합 완료
- hostile blocked follow preflight와 interactive action rejection tests 통과

## 3.10 Phase 9. CLI Surface v0

목표:

- 코어 런타임을 실제로 호출 가능한 제품 표면으로 노출합니다.

구현 항목:

- `open`
- `snapshot`
- `extract`
- `policy`
- `replay`
- `memory-summary`

완료 조건:

- fixture 기반 명령이 JSON 출력으로 실행 가능
- live `open` / `extract` / `policy`가 acquisition handoff와 함께 실행 가능
- browser-backed `open` / `extract` / `policy`가 sidecar handoff와 함께 실행 가능

필수 테스트:

- fixture open CLI test
- hostile policy CLI test
- browser-backed fixture open CLI test
- browser-backed extract CLI test
- browser-backed hostile policy CLI test
- replay CLI test
- 50-action memory CLI test

현재 상태:

- usable CLI surface 구현 완료
- fixture 경로는 action VM, live 경로는 runtime/acquisition handoff를 사용
- browser 경로는 Playwright sidecar -> observation -> runtime handoff를 사용
- persisted browser session path와 low-risk dynamic actions(`follow`, `paginate`, `expand`) 추가 완료
- Rust CLI tests 통과

## 3.11 Phase 10. Eval Harness v0

목표:

- 제품 성능을 자동으로 판정할 수 있게 합니다.

구현 항목:

- suite runner
- grader
- token/latency counters
- regression reporter
- hostile regression suite

왜 여기서 독립 phase로 두는가:

- 일부 테스트는 각 phase에 내장되지만, 전체 게이트를 판정하는 공통 harness도 별도로 필요합니다.

완료 조건:

- core suites 로컬 실행 가능
- 결과 아티팩트 저장 가능

필수 테스트:

- suite execution smoke test
- grader consistency test
- report artifact generation test

현재 상태:

- contract / fixture / snapshot / evidence / session / policy regression이 자동 실행됨
- generated citation metrics scenario가 `pnpm test`에 연결됨
- generated policy, memory(20/50-step), session scenarios가 `pnpm test`에 연결됨
- generated observation metrics scenario가 `pnpm test`에 연결됨
- generated browser-observation metrics scenario가 `pnpm test`에 연결됨
- generated live-observation, session-synthesis, action-surface, latency-cost, customer-fit 리포트가 `pnpm test`에 연결됨
- `serve`, `serve daemon`, `MCP bridge` smoke test가 eval suite에 연결됨
- public-web benchmark는 기본 `pnpm test`에는 넣지 않고 별도 artifact로 유지

## 3.12 Phase 11. Playwright Fallback Adapter v0

목표:

- static path로 충분하지 않은 페이지에만 dynamic browsing fallback을 붙입니다.

구현 항목:

- stdio JSON-RPC bridge
- browser session lifecycle
- dynamic snapshot acquisition
- adapter protocol tests

왜 늦게 하는가:

- Playwright를 먼저 붙이면 제품이 browser automation으로 기울어집니다.
- 우리 첫 가치는 dynamic browser control이 아니라 observation contract입니다.

완료 조건:

- adapter handshake 성공
- dynamic page에서 fallback snapshot 생성 가능

필수 테스트:

- protocol conformance test
- sidecar process lifecycle test
- basic Playwright integration test

현재 상태:

- Playwright adapter browser-backed snapshot baseline 구현 완료
- stdio JSON-RPC request handler 추가 완료
- `adapter.status`, `browser.snapshot`, `browser.follow`, `browser.paginate`, `browser.expand` protocol tests 통과
- Rust CLI에서 `--browser`와 persisted session path로 adapter subprocess handoff 검증 완료
- representative fixture subset 기준 browser-backed observation metrics baseline 추가 완료
- sidecar process lifecycle smoke는 CLI tests와 직접 실행 검증으로 확보
- browser session persistence는 session-file 기반 persistent browser context까지 구현 완료
- `browser-replay`와 compact session surface 구현 완료
- stateful serve daemon session registry와 multi-tab orchestration 구현 완료

## 3.13 Phase 12. Limited Dynamic Actions

목표:

- 정말 필요한 low-risk action만 순차적으로 허용합니다.

초기 허용 후보:

- click navigation
- select tab
- pagination
- expand collapsed content

아직 보류:

- login
- type credentials
- submit form
- purchase-like action

완료 조건:

- action당 risk model 존재
- action replay 가능
- hostile regression 통과

필수 테스트:

- action success test
- action replay test
- unsafe action block test

현재 상태:

- Playwright adapter protocol에서 `follow` / `paginate` / `expand`를 low-risk dynamic action seed로 노출
- core action VM에서는 interactive action을 계속 차단
- 실제 browser-backed dynamic action replay는 아직 미완

## 4. 절대 순서를 바꾸면 안 되는 부분

아래는 뒤집으면 안 됩니다.

1. contracts before fixtures
2. fixtures before observation quality claims
3. observation before dynamic browsing
4. evidence before research-product claims
5. replay before broad integration
6. policy before write-like actions
7. eval harness before performance claims

## 5. 테스트 전략

## 5.1 테스트 레이어

이 프로젝트는 일반적인 unit/integration/e2e만으로 부족합니다.

필수 테스트 레이어:

- contract tests
- fixture tests
- golden snapshot tests
- property tests
- protocol tests
- replay tests
- hostile tests
- benchmark regression tests

## 5.2 레이어별 역할

### Contract Tests

무엇을 막는가:

- Rust/TS schema drift
- invalid payload acceptance

도구:

- AJV
- JSON examples

### Fixture Tests

무엇을 막는가:

- 입력 데이터 품질 저하
- grading ambiguity

### Golden Snapshot Tests

무엇을 막는가:

- observation compiler 회귀
- boilerplate suppression 악화

주의:

- golden update는 의도적 변경일 때만 허용합니다.

### Property Tests

무엇을 막는가:

- ref instability
- compaction invariant 파괴
- ordering nondeterminism

초기 대상:

- stable ref generation
- diff invariants
- compaction invariants

### Protocol Tests

무엇을 막는가:

- stdio JSON-RPC 경계 파손
- sidecar handshake mismatch

### Replay Tests

무엇을 막는가:

- 같은 transcript가 다른 결과를 내는 문제

### Hostile Tests

무엇을 막는가:

- prompt injection 성공
- hidden instruction leakage
- unsafe navigation

### Benchmark Regression Tests

무엇을 막는가:

- 토큰 절감 악화
- citation precision 저하
- latency 악화

## 5.3 머지 전 필수 규칙

새 기능은 아래 없이는 완료로 보지 않습니다.

1. boundary contract 또는 schema 영향 분석
2. fixture 추가 또는 기존 fixture 영향 검토
3. 최소 1개 이상의 회귀 방지 테스트
4. replay 또는 hostile 영향 검토

## 5.4 측정 지표

모든 단계에서 아래 지표 중 관련 항목을 최소 하나 이상 남겨야 합니다.

- tokens per snapshot
- tokens per task
- fact recall
- citation precision
- unsupported claim rate
- replay success
- latency p50/p95
- unsafe action rate

## 5.5 현재 환경 기준의 테스트 가능 범위

현재 이 작업 환경에서는:

- Node 기반 contract/layout 검증 가능
- TS lint/typecheck/test는 dependency install 후 가능
- Rust check/test는 toolchain 설치 전까지 불가

따라서 당장 가능한 첫 검증 루프는 아래입니다.

1. schema 추가
2. AJV validation test
3. fixture + grader wiring
4. manifest regeneration

## 6. 지금 당장 시작할 실제 순서

다음 순서로 진행합니다.

1. 첫 schema 7종 작성
2. schema validation tests 작성
3. fixture metadata 구조 정의
4. static + hostile fixture seed 추가
5. observation compiler 입력/출력 인터페이스 고정
6. golden snapshot baseline 생성
7. evidence extraction baseline 추가

## 7. 시작 전 체크포인트

다음 중 하나라도 비어 있으면 구현보다 먼저 정의합니다.

- snapshot block shape
- ref stability rule
- replay transcript shape
- hostile fixture taxonomy
- grading rule shape

## 8. 변경 이력

### 2026-03-14

- 초기 구현 순서와 테스트 전략 고정
- research wedge 기준으로 read-only first 순서 정의
- dynamic fallback을 후순위로 이동
- schema baseline, fixture seed, smoke tests 기준 반영
