# Context Map

- Status: `Fixed`
- Last Updated: `2026-04-06`
- Scope: `DDD-lite bounded context and integration map`

## 1. 목적

이 문서는 `touch-browser`의 bounded context와 context 사이의 연결 방식을 고정합니다.

이 문서의 역할:

- 저장소 경계와 도메인 경계를 1:1로 맞춘다.
- upstream/downstream 관계를 명시한다.
- published language와 anti-corruption layer 위치를 고정한다.

## 2. 외부 기준

이 context map은 아래 기준을 참고해 정의합니다.

- Eric Evans, *DDD Reference*
- Martin Fowler, *Bounded Context*
- Microsoft Learn, *Use domain analysis to model microservices*

## 3. Context Classification

| Context | Type | Primary Location | Responsibility |
| --- | --- | --- | --- |
| Contracts | Published Language | `contracts` and `core/crates/contracts` | schema, shared DTO, serialization contract |
| Observation | Core Domain | `core/crates/observation` | DOM normalization, stable ref, token-budgeted snapshot |
| Evidence | Core Domain | `core/crates/evidence` | claim-evidence linking, citation, contradiction handling |
| Memory | Core Domain | `core/crates/memory` | session memory, compaction, replay-oriented summaries |
| Policy | Core Domain | `core/crates/policy` | trust boundary, approval, risk classification |
| Action VM | Core Domain | `core/crates/action-vm` | typed action execution surface, failure taxonomy |
| Acquisition | Supporting Subdomain | `core/crates/acquisition` | fetch, redirect, cache, content acquisition |
| Eval | Supporting Subdomain | `evals` | regression benchmark, adversarial benchmark, harness |
| CLI Application | Application / Transport | `core/crates/cli` | orchestration, interface, ports and adapters |
| Playwright Adapter | External Adapter | `adapters/playwright` | browser execution, dynamic page action bridge |
| MCP Bridge | External Integration | `scripts/touch-browser-mcp-bridge.mjs` | remote agent bridge over stdio JSON-RPC |
| External Web | External System | outside repository | live websites, search engines, hostile pages |

## 4. Context Relationships

| Upstream | Downstream | Integration Pattern | Boundary Rule |
| --- | --- | --- | --- |
| Contracts | Observation / Evidence / Memory / Policy / Action VM / CLI / Playwright Adapter / MCP Bridge | Published Language | schema and shared DTO are canonical |
| Acquisition | Observation | Conformist | acquired HTML is normalized before domain use |
| Observation | Evidence | Customer-Supplier | evidence logic consumes stable ref and normalized blocks |
| Policy | CLI Application | Customer-Supplier | CLI orchestrates approvals, policy owns risk logic |
| Action VM | CLI Application | Customer-Supplier | CLI invokes typed actions, action semantics stay typed |
| CLI Application | Playwright Adapter | Ports and Adapters | adapter details stay behind infrastructure ports |
| CLI Application | MCP Bridge | Open Host Service | JSON-RPC transport may not redefine domain meaning |
| Playwright Adapter | External Web | Anti-Corruption Layer | raw browser and DOM state must be translated before entering core models |

## 5. Published Language And ACL Boundaries

### 5.1 Published Language

`Contracts` context만이 process 간 공용 언어를 소유합니다.

고정 규칙:

- `contracts/schemas/*.schema.json`이 canonical source입니다.
- `touch-browser-contracts`는 published language DTO만 유지합니다.
- compact/read/navigation markdown renderer 같은 presentation policy는 contracts에 두지 않습니다.

### 5.2 Anti-Corruption Layers

ACL이 필요한 위치:

- `Playwright Adapter -> CLI Application`
- `External Web -> Acquisition / Observation`
- `MCP Bridge -> CLI Application`

고정 규칙:

- raw DOM, Playwright request/response shape, external JSON-RPC envelope는 내부 도메인 모델과 직접 섞지 않습니다.
- 번역은 adapter 또는 infrastructure에서 끝내고, application은 typed port 계약만 봅니다.

## 6. Human And AI Ownership

사람이 결정할 것:

- 어떤 context를 core domain으로 승격할지
- external system과 어느 수준까지 conformist로 갈지
- published language versioning 정책

AI가 강하게 자동화할 것:

- context map 문서 누락 감지
- contracts와 presentation 정책 혼합 감지
- application 레이어 의존성 누수 감지
- CI에서 context boundary 회귀 감지

## 7. Completion Gate

이 문서는 아래 조건을 만족할 때 완료로 간주합니다.

1. 모든 핵심 context가 실제 경로와 함께 문서화되어 있다.
2. published language와 presentation policy가 분리되어 있다.
3. ports-and-adapters 방향이 `pnpm run architecture:check`에서 자동 검증된다.
4. 품질 게이트는 [DDD_COMPLETION_CRITERIA.md](DDD_COMPLETION_CRITERIA.md)와 SonarQube workflow가 함께 강제한다.

## 8. Sources

- [Eric Evans, DDD Reference](https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf)
- [Martin Fowler, Bounded Context](https://martinfowler.com/bliki/BoundedContext.html)
- [Microsoft Learn, Use Domain Analysis to Model Microservices](https://learn.microsoft.com/en-us/azure/architecture/microservices/model/domain-analysis)
