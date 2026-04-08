# Sonar SAFE Baseline

- Status: `Fixed`
- Last Updated: `2026-04-06`
- Scope: `SonarQube safe exclusions for intentional orchestration and mirrored boundary glue`

## 1. 목적

이 문서는 `touch-browser`에서 SonarQube가 지적한 항목 중
즉시 분해하거나 제거하는 것이 오히려 경계와 추적성을 해치는 항목을 `SAFE`로 기록합니다.

핵심 원칙:

- `SAFE`는 "무시"가 아니라 "의도와 한계를 기록한 예외"입니다.
- 보안 취약점은 `SAFE`로 처리하지 않습니다.
- 이 문서는 구조적으로 필요한 대칭성, boundary glue, orchestration decision table만 다룹니다.

## 2. 현재 보안 상태

2026-04-06 기준 Sonar API 확인 결과:

- `touch-browser` project exists
- `Security Hotspots`: `0`

따라서 현재 SAFE 범위는 보안 취약점이 아니라
duplication 및 maintainability 규칙의 구조적 예외에 한정합니다.

## 3. SAFE Issue Rules

### 3.1 `rust:S107` on `core/crates/cli/src/application/ports.rs`

사유:

- `build_browser_cli_session`은 application port contract입니다.
- 이 함수의 파라미터는 브라우저 세션 구성 경계를 그대로 드러내는 published application vocabulary입니다.
- 이를 하나의 임의 bag struct로 합치면 호출부 의미가 숨겨지고, port 계약 가독성이 오히려 떨어집니다.

SAFE 결론:

- `rust:S107`은 이 파일에서 SAFE입니다.

### 3.2 `rust:S3776` on `core/crates/cli/src/interface/serve_runtime/session_handlers.rs`

사유:

- `combine_session_synthesis_reports`는 tab/session synthesis aggregation rule을 한 곳에 고정하는 함수입니다.
- 과도한 분해는 claim merge 규칙과 citation merge 규칙을 여러 helper로 흩어지게 만듭니다.
- 현재 책임은 단일 aggregation boundary로 읽히며, 테스트도 그 단위에 맞춰져 있습니다.

SAFE 결론:

- 이 orchestration function의 complexity 경고는 SAFE입니다.

### 3.3 `rust:S3776` on `core/crates/cli/src/application/policy_support.rs`

사유:

- checkpoint provider hint는 provider-specific risk hint decision table입니다.
- 각 provider별 분기 자체가 제품 규칙이며, 이를 쪼개면 규칙이 파일 전반에 분산됩니다.

SAFE 결론:

- provider hint decision table complexity는 SAFE입니다.

### 3.4 `rust:S3776` on `core/crates/cli/src/application/search_support.rs`

사유:

- search result URL normalization은 search-engine-specific rewrite rule table입니다.
- Google redirect unwrap, Brave passthrough, YouTube canonicalization은 URL policy boundary로 함께 읽혀야 합니다.

SAFE 결론:

- URL normalization decision table complexity는 SAFE입니다.

### 3.5 `rust:S3776` on `core/crates/cli/src/application/session_reporting.rs`

사유:

- verifier adjudication은 `evidence-supported / contradicted / insufficient / needs-more-browsing` 판정 규칙을 한 곳에 유지해야 합니다.
- 규칙을 과도하게 분해하면 verdict, reason, next-action 정합성이 오히려 추적하기 어려워집니다.

SAFE 결론:

- verifier adjudication decision table complexity는 SAFE입니다.

### 3.6 `rust:S2208` wildcard imports on boundary glue files

대상:

- `core/crates/cli/src/infrastructure/app_ports.rs`
- `core/crates/cli/src/infrastructure/browser_runtime.rs`
- `core/crates/cli/src/main.rs`
- `core/crates/cli/src/application/search_support.rs`

사유:

- 이 파일들은 facade, adapter bridge, parser trait import처럼 넓은 표면을 연결하는 glue layer입니다.
- 특히 crate root re-export와 adapter DTO mirror는 explicit import로 전부 늘어놓을수록 drift와 merge churn이 커집니다.
- 이 규칙은 도메인 purity를 깬 것이 아니라 glue layer의 import style을 지적한 것입니다.

SAFE 결론:

- 해당 resource의 `rust:S2208`은 SAFE입니다.

## 4. SAFE Duplication Exclusions

다음 파일은 duplication metric에서 제외합니다.

- `core/crates/cli/src/interface/browser_session_parser.rs`
- `core/crates/cli/src/application/browser_session_actions.rs`
- `core/crates/cli/src/interface/search_command_parser.rs`
- `core/crates/cli/src/infrastructure/browser_models.rs`
- `core/crates/cli/src/interface/session_command_parser.rs`

사유:

- 이 파일들은 command parser / adapter action wrapper / browser DTO bridge 성격입니다.
- 의도적으로 대칭적인 surface를 제공하므로, Sonar CPD가 중복으로 보는 라인 상당수가 실제로는 public surface symmetry입니다.
- 이 영역의 중복은 domain logic duplication이 아니라 transport symmetry입니다.

SAFE 결론:

- 위 파일은 `sonar.cpd.exclusions` 대상으로 유지합니다.

## 5. 검토 규칙

다음 조건 중 하나라도 깨지면 SAFE에서 제거해야 합니다.

1. 동일 규칙이 domain core 로직으로 번지기 시작한 경우
2. duplication이 transport symmetry를 넘어 business rule duplication으로 번진 경우
3. 실제 security hotspot 또는 vulnerability가 생긴 경우

## 6. 근거

- 현재 SonarQube quality gate 실패 원인은 `new_duplicated_lines_density`와 `new_violations`입니다.
- security hotspot은 `0`입니다.
- 따라서 SAFE baseline은 “보안 예외”가 아니라 “구조적 orchestration/transport 예외”로만 한정합니다.
