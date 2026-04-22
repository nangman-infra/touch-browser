# Evidence Rules

`touch-browser` returns page-local evidence, not world truth.

Preserve these distinctions:

- `evidence-supported`
- `contradicted`
- `insufficient-evidence`
- `needs-more-browsing`

Required behavior:

- inspect support snippets before reuse
- preserve `confidenceBand`
- preserve `reviewRecommended`
- prefer unresolved outcomes to weak certainty

Read the detailed source contracts in:

- [../../../README.md](../../../README.md)
- [../../../doc/TOUCH_BROWSER_SKILL_SPEC.md](../../../doc/TOUCH_BROWSER_SKILL_SPEC.md)
