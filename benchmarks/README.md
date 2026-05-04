# Benchmarks

Latest generated proof signals from the local `2026-05-05` rerun:

| Benchmark | Signal |
| --- | --- |
| Public web benchmark | `4/4` task-proof claims evidence-supported on the IANA/RFC public sample set |
| Real-user research benchmark | average supported claim rate `1.00` across `3` MCP-driven public-doc scenarios |
| Adversarial benchmark | verified exact verdict accuracy `1.00`; raw exact verdict accuracy `0.80`; unsafe auto-answer count `0` |
| Citation metrics | classification, citation, unsupported-claim, and support-reference precision/recall `1.00` across `34` fixtures |
| Tool comparison benchmark | extract false-positive rate `0.17` vs markdown-baseline false-positive rate `0.33` on plausible negative claims |
| Latency/cost metrics | compact token cost ratio `0.14`; browser open latency multiplier `136.19` in the local sample |
| Evidence-grounded web research benchmark | current seed `61` claim/fixture units with status `seed-validated`; target corpus `1000` claims |

These numbers come from generated `fixtures/scenarios/*/report.json` artifacts after running the benchmark fixture scripts. Live public-web reruns can move when upstream pages or network conditions change.

- [tool comparison benchmark](../doc/TOOL_COMPARISON_BENCHMARK_SPEC.md): `touch-browser` vs a reproducible markdown-only baseline on official public sources
- [evidence-grounded web research benchmark](../doc/EVIDENCE_GROUNDED_WEB_RESEARCH_BENCHMARK_SPEC.md): category-level benchmark wrapper for evidence support, citations, abstention, adversarial routing, and false-positive control
- [adversarial benchmark](../doc/ADVERSARIAL_BENCHMARK_SPEC.md): numeric mismatch, contradiction, and `needs-more-browsing` cases on official docs
- [public web benchmark](../doc/PUBLIC_WEB_BENCHMARK_SPEC.md): live public-doc coverage and token reduction
- [real-user research benchmark](../doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md): MCP-driven multi-tab research proof
- [AWS page-type benchmark](../doc/AWS_PAGE_TYPE_BENCHMARK_SPEC.md): AWS Docs page archetypes with auto vs browser capture, latency, and main-only usefulness
