# Fixtures

이 디렉터리는 연구형 browsing과 hostile browsing 평가를 위한 재현 가능한 입력을 보관합니다.

현재는 seed corpus만 포함합니다.

카테고리:

- `research/static-docs`
- `research/navigation`
- `research/citation-heavy`
- `research/hostile`
- `scenarios/read-only-pricing`
- `scenarios/memory-20-step`

각 fixture는 아래 파일을 가질 수 있습니다.

- `index.html`
- `fixture.json`
- `expected-snapshot.json`
- `expected-evidence.json`

`fixture.json`은 현재 아래 메타데이터를 포함합니다.

- source uri
- expected snapshot path
- expected evidence path
- claim checks

향후 추가 예정:

- grading rules
