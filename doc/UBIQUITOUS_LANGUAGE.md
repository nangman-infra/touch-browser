# Ubiquitous Language

- Status: `Fixed`
- Last Updated: `2026-04-06`
- Scope: `shared domain vocabulary and presentation separation`

## 1. 목적

이 문서는 코드, schema, 문서, benchmark에서 같은 단어가 같은 뜻으로 쓰이게 고정합니다.

핵심 원칙:

- 같은 단어는 같은 뜻만 가진다.
- 다른 뜻이면 다른 단어를 쓴다.
- published language와 presentation 용어를 분리한다.

## 2. 외부 기준

- Eric Evans, *DDD Reference*
- Martin Fowler, *Ubiquitous Language*
- Microsoft Learn, *Use tactical DDD to design microservices*

## 3. Canonical Terms

| Term | Meaning | Owner Context | Forbidden Drift |
| --- | --- | --- | --- |
| Snapshot | token-budgeted normalized page document | Observation / Contracts | raw DOM, rendered markdown와 혼용 금지 |
| Stable Ref | snapshot block을 재참조하는 안정 식별자 | Observation / Contracts | CSS selector, Playwright locator와 혼용 금지 |
| Evidence | claim을 지지하거나 반박하는 normalized support | Evidence / Contracts | LLM opinion, unverifiable summary와 혼용 금지 |
| Claim | 검증 대상이 되는 명시 진술 | Evidence / Runtime | note, hypothesis와 혼용 금지 |
| Verification Verdict | verifier가 낸 판정 | Evidence / Contracts | final claim verdict와 동일시 금지 |
| Policy | 위험과 승인 경계를 판단하는 규칙 | Policy | UI preference, formatting rule과 혼용 금지 |
| Session | 연속된 탐색 상태와 기록 단위 | Runtime / Memory | browser tab 자체와 동일시 금지 |
| Action Result | typed action execution 결과 | Action VM / Contracts | raw adapter payload와 혼용 금지 |
| Search Report | 검색 결과와 next-action 힌트의 typed report | CLI Application / Contracts | 브라우저 snapshot과 혼용 금지 |

## 4. Presentation Terms

아래 용어는 published language가 아니라 presentation policy입니다.

| Term | Meaning | Allowed Location |
| --- | --- | --- |
| Compact View | 빠른 훑어보기용 압축 표현 | `/Volumes/WD/Developments/touch-browser/core/crates/cli/src/application/presentation_support.rs` |
| Read View | 읽기 최적화 markdown 표현 | `/Volumes/WD/Developments/touch-browser/core/crates/cli/src/application/presentation_support.rs` |
| Navigation View | navigation noise를 분리한 compact/read 표현 | `/Volumes/WD/Developments/touch-browser/core/crates/cli/src/application/presentation_support.rs` |
| Session Synthesis Markdown | 세션 합성 결과의 human-readable markdown | `/Volumes/WD/Developments/touch-browser/core/crates/cli/src/application/presentation_support.rs` |

고정 규칙:

- presentation helper는 `touch-browser-contracts`에 두지 않습니다.
- contracts는 DTO와 serialization contract만 유지합니다.
- markdown formatter, compact renderer, read renderer는 application의 presentation support로 둡니다.

## 5. Naming Rules

1. schema field 이름과 Rust/TS 타입 이름은 가능한 한 같은 의미를 유지합니다.
2. alias는 backwards compatibility일 때만 허용합니다.
3. `supported` 같은 축약 표현보다 `evidence-supported` 같은 명시 표현을 우선합니다.
4. 외부 adapter 용어는 내부 domain 용어로 번역한 뒤 사용합니다.

## 6. Human And AI Ownership

사람이 결정할 것:

- 새 용어를 기존 용어 확장으로 볼지, 새 bounded context로 볼지
- backwards compatibility를 위해 어떤 alias를 유지할지

AI가 자동화할 것:

- contracts에 presentation API 재유입 감지
- 문서 필수 용어 누락 감지
- schema와 코드 naming drift 탐지 후보 제시

## 7. Completion Gate

완료 기준:

1. 핵심 용어가 이 문서에 존재한다.
2. contracts crate에는 presentation API가 없다.
3. `pnpm run architecture:check`가 금지 패턴을 자동 검증한다.
4. regression suite와 SonarQube gate가 naming/presentation drift 회귀를 막는다.

## 8. Sources

- [Eric Evans, DDD Reference](https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf)
- [Martin Fowler, Ubiquitous Language](https://martinfowler.com/bliki/UbiquitousLanguage.html)
- [Microsoft Learn, Use Tactical DDD to Design Microservices](https://learn.microsoft.com/en-us/azure/architecture/microservices/model/tactical-domain-driven-design)
