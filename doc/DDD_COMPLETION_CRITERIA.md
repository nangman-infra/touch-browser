# DDD Completion Criteria

- Status: `Fixed`
- Last Updated: `2026-04-06`
- Scope: `DDD-lite definition of done and automated quality gates`

## 1. 목적

이 문서는 `touch-browser`에서 DDD-lite 완료를 선언하는 기준을 고정합니다.

중요한 전제:

- 이 저장소는 `풀 DDD`가 아니라 `DDD-lite + Ports and Adapters + Contract-first`를 채택합니다.
- 완료 기준은 구조, 언어, 불변식, 계약, 품질 게이트가 자동으로 유지되는 상태입니다.
- 6번 완료 기준은 `SonarQube quality gate 통과`로 정의합니다.

## 2. 외부 기준

- Eric Evans, *DDD Reference*
- Martin Fowler, *Bounded Context*
- Martin Fowler, *Ubiquitous Language*
- Martin Fowler, *Anemic Domain Model*
- Vaughn Vernon, *Effective Aggregate Design*
- Microsoft Learn, *Use domain analysis to model microservices*
- Microsoft Learn, *Use tactical DDD to design microservices*
- SonarSource Docs, *Rust*
- SonarSource Docs, *Analysis parameters*

## 3. Definition Of Done

| No. | Criterion | Repository Rule | Automated Gate |
| --- | --- | --- | --- |
| 1 | Explicit Context Map | bounded context와 integration pattern이 문서화되어 있어야 한다 | `pnpm run architecture:check`가 `doc/CONTEXT_MAP.md` 존재와 핵심 context 용어를 검사 |
| 2 | Ubiquitous Language | 핵심 용어와 presentation 용어가 분리돼 있어야 한다 | `pnpm run architecture:check`가 `doc/UBIQUITOUS_LANGUAGE.md` 존재와 필수 용어를 검사 |
| 3 | Application Purity | application은 infrastructure concrete type, shell spawn, raw JSON 조립을 직접 가지지 않는다 | `pnpm run architecture:check`가 `crate::infrastructure::`, `default_cli_ports(`, `Command::new(`, `Stdio::`, `json!(`, `use crate::*;`를 차단 |
| 4 | Invariant Ownership | 세션, evidence, policy, action 관련 핵심 규칙은 도메인 또는 typed application service에 귀속된다 | 빠른 merge gate는 `pnpm run quality:ci`, 로컬 확장 검증은 `pnpm run quality:full`이 replay, policy, evidence, session 회귀를 강제 |
| 5 | Published Language Separation | contracts는 published language만 유지하고 presentation policy는 application으로 분리한다 | `pnpm run contracts:check`, `pnpm run contracts:manifest`, `pnpm run architecture:check`가 contracts의 public renderer 재유입을 차단 |
| 6 | Continuous Quality Gate | 품질 완료는 SonarQube quality gate 통과로 정의한다 | GitHub Actions `sonar.yml`은 `pnpm run quality:ci`만 merge-blocking으로 실행하고, Sonar는 그 뒤 `sonar.qualitygate.wait=true`로 최신 분석 결과를 기다린다 |

## 4. Automation Map

### 4.1 Local Gate

개발 중 로컬 기준:

```bash
pnpm run architecture:check
pnpm run quality:full
```

### 4.2 CI Gate

CI 기준:

1. `quality-checks` job은 merge를 막아야 하는 필수 검증만 담은 `pnpm run quality:ci`를 통과해야 합니다.
2. 이 빠른 gate에는 `lint`, `fmt`, `typecheck`, `clippy`, 전체 Rust 테스트, Playwright adapter gate, 최소 `serve/MCP/CLI` smoke 검증만 포함됩니다.
3. 문서 문구 검증, fixture-heavy eval, proof/benchmark 계열 검증은 로컬 `pnpm run quality:full`에서 확인합니다.
4. 그 다음 `sonarqube` job이 `pnpm run quality:sonar-reports`로 Clippy JSON report를 생성합니다.
5. Sonar scan은 `sonar.qualitygate.wait=true`로 품질 게이트 결과를 기다립니다.
6. Quality Gate가 실패하면 DDD-lite 완료 기준 6번이 실패합니다.

## 4.3 Sonar Official Verification

- Default shell env is not assumed to contain Sonar credentials.
- Load local ignored credentials from `/Volumes/WD/Developments/nangman-infra/.env.sonar.local` when available.
- Query:
  - quality gate
  - latest analysis revision
  - unresolved issues
  - hotspots
- Not done until:
  - latest analysis revision == pushed SHA
  - Quality Gate == OK
  - new_violations == 0
  - unresolved issues == 0
  - hotspots == 0

## 5. Human And AI Ownership

사람이 결정할 것:

- 어떤 불변식을 core domain에 둘지
- 위험 모델과 정책 모델의 실제 제품 기준
- SonarQube에서 어떤 quality profile과 quality gate를 운영할지

AI가 자동화할 것:

- 문서 누락 탐지
- 아키텍처 금지 패턴 탐지
- contracts/presentation 혼합 감지
- CI와 SonarQube gate 연결 유지

## 6. Completion Declaration

아래 조건을 모두 만족하면 이 저장소의 DDD-lite 완료로 선언합니다.

1. [CONTEXT_MAP.md](CONTEXT_MAP.md)가 최신 구조를 반영한다.
2. [UBIQUITOUS_LANGUAGE.md](UBIQUITOUS_LANGUAGE.md)가 최신 핵심 용어를 반영한다.
3. `pnpm run architecture:check`가 통과한다.
4. `pnpm run quality:ci`가 통과한다.
5. 로컬 확장 검증이 필요할 때 `pnpm run quality:full`로 문서/fixture-heavy 검증까지 확인한다.
6. SonarQube scan이 실행되고 `latest analysis revision == pushed SHA`, `Quality Gate == OK`, `new_violations == 0`, `unresolved issues == 0`, `hotspots == 0`를 만족한다.

## 7. Sources

- [Eric Evans, DDD Reference](https://www.domainlanguage.com/wp-content/uploads/2016/05/DDD_Reference_2015-03.pdf)
- [Martin Fowler, Bounded Context](https://martinfowler.com/bliki/BoundedContext.html)
- [Martin Fowler, Ubiquitous Language](https://martinfowler.com/bliki/UbiquitousLanguage.html)
- [Martin Fowler, Anemic Domain Model](https://martinfowler.com/bliki/AnemicDomainModel.html)
- [Vaughn Vernon, Effective Aggregate Design](https://www.dddcommunity.org/library/vernon_2011/)
- [Microsoft Learn, Use Domain Analysis to Model Microservices](https://learn.microsoft.com/en-us/azure/architecture/microservices/model/domain-analysis)
- [Microsoft Learn, Use Tactical DDD to Design Microservices](https://learn.microsoft.com/en-us/azure/architecture/microservices/model/tactical-domain-driven-design)
- [SonarSource Docs, Rust](https://docs.sonarsource.com/sonarqube-community-build/analyzing-source-code/languages/rust/)
- [SonarSource Docs, Analysis Parameters](https://docs.sonarsource.com/sonarqube-server/2025.3/analyzing-source-code/analysis-parameters/)
