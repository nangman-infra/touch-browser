# AWS Page-Type Benchmark Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-12`
- Scope: `AWS Docs page archetypes for adaptive capture quality, latency balance, and diagnostics coverage`

## 1. Overview

This benchmark exists to answer a product question that the broader JS benchmark does not answer:

- when an AI researcher opens real AWS documentation, does `touch-browser` choose a useful capture path quickly enough, and can it explain that choice with structured diagnostics?

The point is not whether one AWS page can be forced to work. The point is whether a cross-section of AWS page types behaves predictably under the same adaptive capture policy.

## 2. What Is Measured

For each sample:

- auto `open` source type, latency, and diagnostics
- forced-browser `open --browser` source type, latency, and diagnostics
- `read-view --main-only` usefulness through must-contain recall
- whether forced browser produced a richer capture than auto

## 3. Sample Set

The current live benchmark includes:

- AWS Lambda quotas
- Amazon S3 `ListObjectsV2` API reference
- Amazon S3 examples using SDK for JavaScript (v3)
- AWS Prescriptive Guidance patterns
- Amazon S3 Files guide
- CloudFormation Template Reference guide

These are intentionally mixed across:

- quota and service-guide pages
- API reference pages
- code example pages
- prescriptive guidance pages
- newer AWS doc shells and storage product pages

## 4. Artifacts

Generated report:

- [report.json](../fixtures/scenarios/aws-page-type-benchmark/report.json)

Runner:

- [generate-aws-page-type-benchmark.mjs](../scripts/generate-aws-page-type-benchmark.mjs)

Run:

- `pnpm run fixtures:aws-page-types`

## 5. Passing Criteria

The benchmark is considered healthy when:

- every sample opens successfully in auto mode
- every sample produces non-empty `read-view --main-only`
- average main-only recall remains high enough to stay useful for research
- diagnostics are present on auto and browser captures
- forced browser is only materially richer on a minority of samples, not the majority

## 6. Why This Benchmark Exists

- JS-rendered benchmarks prove generic app/docs routing
- this benchmark proves page-type robustness in the AWS cohort that real users actually research
- it also records the latency tradeoff of the adaptive readiness probe so quality improvements do not silently become global slowdown

## 7. Limits

- AWS documentation can change without notice
- this is still a curated page-type cohort, not a proof that every AWS page will behave identically
- must-contain texts are intentionally stable phrases, not exhaustive semantic validation
