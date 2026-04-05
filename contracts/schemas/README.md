# Schemas

이 디렉터리의 JSON Schema가 프로세스 간 계약의 기준 소스입니다.

초기 우선순위:

1. snapshot-block.schema.json
2. snapshot-document.schema.json
3. evidence-block.schema.json
4. unsupported-claim.schema.json
5. evidence-report.schema.json
6. action-command.schema.json
7. session-state.schema.json
8. replay-transcript.schema.json
9. json-rpc envelopes

주의:

- `WP-01 Observation Contract`부터는 schema-first 원칙에 따라 스키마를 점진적으로 확장합니다.
- boundary payload는 먼저 schema로 고정하고, 그다음 Rust/TS 소비자를 구현합니다.
