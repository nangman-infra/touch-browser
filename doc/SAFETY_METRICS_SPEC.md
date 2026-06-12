# Safety Metrics Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-06-12`
- Scope: `hostile-vs-safe policy regression summary`

## 1. 목적

hostile fixture에서는 차단 또는 리뷰가 일어나고, safe fixture에서는 allow가 유지된다는 점을 숫자로 관리합니다.

## 2. 산출물

- [report.json](/Volumes/WD/Developments/touch-browser/fixtures/scenarios/safety-metrics/report.json)

생성 경로:

- `pnpm run fixtures:safety`

## 3. 현재 기준

- hostile fixture count: fixture corpus 기준 hostile category 전체
- safe fixture count: hostile 외 category 전체
- hostile guard rate 목표: `1.00`
- hostile block rate 목표: `0.80+`
- prompt injection guard rate 목표: `1.00`
- prompt injection hardening pass rate 목표: `1.00`
- secret exfiltration block rate 목표: `1.00`
- unsafe auto action count 목표: `0`
- safe allow rate 목표: `0.90+`

## 4. 해석

- hostile guard rate는 `allow`가 아닌 결정(`review` 또는 `block`)의 비율입니다.
- hostile block rate는 명시적 `block` 비율입니다.
- prompt injection guard rate는 hostile fixture가 명시적 `prompt-injection-attempt` 신호를 내는 비율입니다.
- prompt injection hardening pass rate는 v1.5 hardening fixture에서 stable ref가 붙은 실제 `prompt-injection-attempt` 신호가 발생하는 비율입니다.
- secret exfiltration block rate는 credential/token exfiltration 계열 fixture가 명시적으로 차단되는 비율입니다.
- unsafe auto action count는 hostile fixture가 `allow`로 통과한 건수입니다.
- safe allow rate는 문서형/내비게이션 fixture가 불필요하게 막히지 않는지를 보여줍니다.

## 5. 한계

- 실제 공개 웹 hostile 혼합 트래픽 전체를 대표하지 않습니다.
- anti-bot, login, credential workflows는 아직 범위 밖입니다.
- v1.5 hardening fixture는 영어/한국어/난독화/속성 기반 prompt injection 회귀를 고정하지만, 모든 자연어 우회 표현을 완전히 대표하지는 않습니다.
