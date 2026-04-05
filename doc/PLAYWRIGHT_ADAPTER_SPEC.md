# Playwright Adapter Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-03-18`
- Scope: `stdio JSON-RPC fallback adapter surface`

## 1. 목적

이 문서는 현재 Playwright adapter의 구현과 CLI 연결 지점을 고정합니다.

현재 범위:

- stdio JSON-RPC request handling
- browser-backed `browser.snapshot`
- `touch-browser --browser` 경로와의 handoff
- adapter capability discovery
- low-risk and allowlisted interactive action execution (`browser.follow`, `browser.click`, `browser.type`, `browser.submit`, `browser.paginate`, `browser.expand`)
- `contextDir` 기반 persistent browser context 재사용

아직 범위 밖:

- screenshot / DOM / accessibility capture
- standalone browser daemon

## 2. 현재 메서드

- `adapter.status`
- `browser.snapshot`
- `browser.follow`
- `browser.click`
- `browser.type`
- `browser.submit`
- `browser.paginate`
- `browser.expand`

## 3. 현재 의미

- `adapter.status`
  - adapter 상태와 capability 목록을 반환
- `browser.snapshot`
  - Playwright Chromium을 실제로 launch해 `url`, `html`, 또는 `contextDir` 입력을 캡처
  - `contextDir`만 주어져도 persistent context를 재사용해 현재 페이지 상태를 다시 읽을 수 있으며, 이 경로를 CLI `refresh`가 사용합니다.
  - `finalUrl`, `title`, `visibleText`, `html`, `linkCount`, `buttonCount`, `inputCount`를 반환
- `browser.paginate`
  - `url` 또는 `html`을 열고 next/prev pagination target을 실제로 click
  - updated `html`, `visibleText`, `finalUrl`, `clickedText`, `page`를 반환
- `browser.follow`
  - `url` 또는 `html`을 열고 지정된 링크 target text/href/ref에 대응하는 anchor를 실제로 click
  - CLI stable ref는 `targetText`, `targetHref`, `targetTagName`, `targetDomPathHint`, `targetOrdinalHint` 힌트로 변환되어 duplicate link를 더 정확히 고릅니다.
  - visible locator를 찾지 못하더라도 same-origin/internal `href`가 있으면 안전한 직접 navigation fallback으로 follow를 계속 시도합니다.
  - updated `html`, `visibleText`, `finalUrl`, `clickedText`를 반환
- `browser.click`
  - `url` 또는 `html`을 열고 지정된 button/link/control target을 실제로 click
  - CLI stable ref는 `targetText`, `targetHref`, `targetTagName`, `targetDomPathHint`, `targetOrdinalHint` 힌트로 변환됩니다.
  - updated `html`, `visibleText`, `finalUrl`, `clickedText`를 반환
- `browser.type`
  - `url` 또는 `html`을 열고 지정된 input/textarea/contenteditable target에 값을 입력합니다.
  - CLI stable ref는 `targetText`, `targetTagName`, `targetDomPathHint`, `targetOrdinalHint`, `targetName`, `targetInputType` 힌트로 변환됩니다.
  - `sensitive` 입력은 adapter 결과에 raw value를 남기지 않습니다.
  - updated `html`, `visibleText`, `finalUrl`, `typedLength`를 반환
- `browser.submit`
  - `url` 또는 `html`을 열고 지정된 form 또는 submit control을 실제로 제출합니다.
  - CLI stable ref는 `targetText`, `targetTagName`, `targetDomPathHint`, `targetOrdinalHint` 힌트로 변환됩니다.
  - `prefill`이 주어지면 submit 전에 같은 browser pass 안에서 non-sensitive 또는 supervised secret input 값을 다시 채웁니다.
  - form target이면 `requestSubmit()` 우선, 아니면 submit control click으로 처리합니다.
  - submit 직후 navigation race가 발생하더라도 최종 DOM과 URL을 다시 수집해 반환합니다.
  - updated `html`, `visibleText`, `finalUrl`을 반환
- `browser.expand`
  - `url` 또는 `html`을 열고 지정된 target text/ref에 대응하는 버튼/링크를 실제로 click
  - CLI stable ref는 `targetText`, `targetTagName`, `targetDomPathHint`, `targetOrdinalHint` 힌트로 변환되어 duplicate button/control을 더 정확히 고릅니다.
  - updated `html`, `visibleText`, `finalUrl`, `clickedText`를 반환

## 4. 현재 보장

- transport는 `stdio-json-rpc`
- malformed request는 JSON-RPC error로 거절
- browser-backed snapshot은 headless Chromium을 기본 사용
- Rust CLI에서 adapter subprocess를 직접 호출해 semantic snapshot/evidence/policy 루프에 연결 가능
- dynamic action은 `limitedDynamicAction: true`로 명시
- CLI-managed persisted session이 current DOM/URL과 browser context dir을 저장하므로 multi-command browser loop와 native browser replay에 연결 가능
- persistent context만 가진 세션도 `browser.snapshot` 재호출로 DOM/URL을 다시 동기화할 수 있습니다.
- 같은 `contextDir`에 대한 concurrent adapter 요청은 cross-process lock으로 직렬화됩니다.
- duplicate text/href candidate는 stable ref ordinal 힌트까지 사용해 DOM order 기준으로 재선택 가능
- hidden duplicate candidate는 visible filter로 제외됩니다.
- visible locator를 찾지 못하는 live site nav는 same-origin `href` fallback으로 일부 복구됩니다.
- input locator는 placeholder/name/type/value/aria-label 조합으로도 점수화되어 login/search form에서 더 안정적으로 선택됩니다.

## 5. 현재 검증 범위

Vitest:

- adapter status contract
- status request handling
- browser-backed snapshot request handling
- follow/paginate/expand handling
- click/type/submit handling
- duplicate link/button disambiguation handling
- safe href follow fallback handling
- malformed request rejection

Rust CLI:

- browser-backed fixture open
- browser-backed extract
- browser-backed hostile policy
- browser session follow / session-extract
- browser session duplicate-follow stable ref ordinal test
- browser session paginate
- browser session double-paginate DOM persistence
- browser session expand / session-extract
- browser replay CLI test
- browser context cleanup test

## 6. 현재 한계

- runtime observation handoff는 완료됨
- low-risk dynamic action은 browser-backed execution과 persistent context replay까지 올라왔지만 standalone daemon과 multi-tab orchestration은 아직 없음
- interactive type/click/submit은 CLI/daemon에서 allowlist, policy preflight, supervised ack-risk를 거친 제한적 지원입니다.
- persistent context live submit은 page reload를 피해서 직전 interactive DOM state를 최대한 유지합니다.
- direct CLI가 같은 persistent context를 연속 호출할 때 profile lock 충돌을 줄이기 위해 adapter가 session directory 단위 lock을 사용합니다.
- adapter는 native DOM node handle에 stable ref를 직접 매핑하지 않고, stable ref를 selector 힌트 집합으로 변환해 사용합니다.
- large live sites에서는 snapshot에 보이는 actionable item과 현재 viewport/DOM의 clickability가 완전히 일치하지 않을 수 있습니다.
- anti-bot/CAPTCHA/MFA를 직접 우회하지는 않으며, 현재 역할은 해당 경계 이후 상태를 안전하게 재동기화하는 것입니다.
