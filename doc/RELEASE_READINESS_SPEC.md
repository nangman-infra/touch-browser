# Release Readiness Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `pilot-readiness gate for touch-browser`

## 1. Overview

This document fixes the internal readiness artifact that classifies the repository as `pilot-ready`, `alpha-ready`, or `incomplete`.

## 2. Artifact

- [report.json](../fixtures/scenarios/release-readiness/report.json)

Generation:

- `pnpm run fixtures:release-readiness`

## 3. Readiness Inputs

- customer-fit baseline
- customer proxy task suite
- safety metrics
- 100-step memory stability
- staged public/trusted workflow
- observation G1 readiness
- operations and security package readiness
- latency-cost baseline
- public proof artifact presence
- real-user public research benchmark
- docs and scripts presence

## 4. Status Meanings

- `pilot-ready`: the internal quality, safety, long-session, mixed-source workflow, observation baseline, ops package, operations docs, public proof, and real-user public benchmark gates all clear the required threshold
- `alpha-ready`: usable before pilot, but still missing reinforcement in public proof, real-user benchmark breadth, observation baseline, or the operations package
- `incomplete`: the repository still misses core internal gates

## 5. Notes

- this is not a GA decision
- real customer production telemetry and support operations are separate from this readiness artifact
- `operationsPackageReady` only covers the self-hosted pilot package, not a managed cloud control plane
