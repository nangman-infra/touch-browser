# Release Readiness Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-06-12`
- Scope: `product-readiness gate for touch-browser`

## 1. Overview

This document fixes the internal readiness artifact that classifies the repository as `product-ready`, `pilot-ready`, `alpha-ready`, or `incomplete`.

## 2. Artifact

- [report.json](../fixtures/scenarios/release-readiness/report.json)

Generation:

- `pnpm run fixtures:release-readiness`

## 3. Readiness Inputs

- customer-fit baseline
- customer proxy task suite
- safety metrics
- prompt injection guard coverage
- prompt injection hardening coverage
- 100-step memory stability
- staged public/trusted workflow
- observation G1 readiness
- operations and security package readiness
- latency-cost baseline
- public proof artifact presence
- real-user public research benchmark
- docs and scripts presence
- installed/bundled CLI smoke for first-run and rejected-action behavior

## 4. Status Meanings

- `product-ready`: all tracked release gates clear, including internal quality, safety, prompt injection guard, long-session, mixed-source workflow, observation baseline, ops package, operations docs, public proof, real-user public benchmark, tool-comparison, and adversarial benchmark readiness
- `pilot-ready`: most tracked release gates clear, but one or more product-readiness gates still require reinforcement before a V1 product release
- `alpha-ready`: usable before pilot, but still missing reinforcement in public proof, real-user benchmark breadth, observation baseline, or the operations package
- `incomplete`: the repository still misses core internal gates

## 5. Notes

- `product-ready` here means a local single-operator product release, not managed SaaS general availability
- real customer production telemetry and support operations are separate from this readiness artifact
- `operationsPackageReady` only covers the self-hosted pilot package, not a managed cloud control plane
- release readiness must exercise the built command surface directly, not only source-level tests, so installed/source CLI drift is caught before publishing
- `promptInjectionGuardReady` requires every hostile fixture to emit an explicit `prompt-injection-attempt` signal, not only a generic hostile-source classification
- `promptInjectionHardeningReady` requires multilingual, obfuscated, and action-attribute prompt-injection fixtures to emit stable-ref-backed `prompt-injection-attempt` signals, same-session historical prompt-injection taint to block later interactive actions, `unsafeAutoActionCount = 0`, and `secretExfiltrationBlockRate = 1.0`
- `adversarialBenchmarkReady` requires `rawUnsafeAutoAnswerCount = 0`, `verifiedUnsafeAutoAnswerCount = 0`, and full review capture, not only final verifier accuracy
