# Fixtures

This directory stores reproducible inputs for research-oriented browsing and hostile-page evaluation.

The current repository includes the seed corpus.

Categories:

- `research/static-docs`
- `research/navigation`
- `research/citation-heavy`
- `research/hostile`
- `scenarios/read-only-pricing`
- `scenarios/memory-20-step`

Each fixture may include:

- `index.html`
- `fixture.json`
- `expected-snapshot.json`
- `expected-evidence.json`

`fixture.json` currently carries:

- source URI
- expected snapshot path
- expected evidence path
- claim checks

Planned additions:

- grading rules
