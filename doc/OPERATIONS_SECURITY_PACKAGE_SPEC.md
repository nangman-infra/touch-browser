# Operations Security Package Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `self-hosted pilot operations and security package`

## 1. Overview

This document defines the minimum operations and security package for running `touch-browser` as a self-hosted pilot.

Included scope:

- container build artifact
- container compose artifact
- environment example
- runtime healthcheck
- secret lifecycle runbook
- telemetry retention and audit runbook
- upgrade and rollback runbook

## 2. Container Runtime

Included artifacts:

- [deploy/Dockerfile](../deploy/Dockerfile)
- [deploy/docker-compose.pilot.yml](../deploy/docker-compose.pilot.yml)
- [deploy/touch-browser.env.example](../deploy/touch-browser.env.example)
- [scripts/pilot-healthcheck.mjs](../scripts/pilot-healthcheck.mjs)

Build:

```bash
docker build -f deploy/Dockerfile -t touch-browser:pilot .
```

Run:

```bash
docker run --rm -i \
  -e TOUCH_BROWSER_TELEMETRY_DB=/data/telemetry.sqlite \
  -e TOUCH_BROWSER_TELEMETRY_SURFACE=serve \
  -v "$(pwd)/output/pilot:/data" \
  touch-browser:pilot \
  target/release/touch-browser serve
```

Compose example:

```bash
docker compose -f deploy/docker-compose.pilot.yml up --build
```

Healthcheck:

- the container healthcheck uses `node scripts/pilot-healthcheck.mjs`
- the healthcheck only verifies a successful `runtime.status` round-trip

## 3. Secret Lifecycle

- direct CLI secrets are stored only in the secret sidecar next to the `--session-file`
- `session-close` should clean the secret sidecar together with the browser context
- daemon-mode secrets stay only in the in-memory daemon secret store and are used through `runtime.session.secret.store` and `runtime.session.typeSecret`
- operators should prefer the daemon secret store over plaintext CLI arguments in production-like pilot environments
- raw secrets should never appear in logs, telemetry, or MCP responses

## 4. Telemetry Retention And Audit

- the default telemetry path is `telemetry.sqlite` and can be overridden with `TOUCH_BROWSER_TELEMETRY_DB`
- the default pilot retention policy is short-lived local retention with optional export before rotation
- minimum audit access paths:
  - `touch-browser telemetry-summary`
  - `touch-browser telemetry-recent --limit <count>`
  - serve `runtime.telemetry.summary`
  - MCP `tb_telemetry_summary`
- copy `telemetry.sqlite` while the service is stopped if a backup is required
- retention and export policy should be made explicit in the operator runbook for each environment

## 5. Upgrade And Rollback

Before upgrade:

- record the current image or binary tag
- back up `telemetry.sqlite`
- keep a copy of the current pilot env file
- run `pnpm test` or at least the minimum smoke gate

Upgrade:

- build the new image or deploy the new binary
- verify runtime status with `scripts/pilot-healthcheck.mjs`
- rerun the reference and staged workflow smoke paths

Rollback:

- return immediately to the previous image or binary
- reconnect the saved `telemetry.sqlite` backup or keep the existing file if that is the safer path
- re-run the `touch-browser serve` healthcheck

## 6. Hardening Baseline

- use allowlists by default for external live browsing
- do not continue auth, MFA, challenge, or high-risk write flows without `checkpoint -> approve`
- keep the pilot container filesystem as immutable as possible outside the telemetry volume
- do not mix benchmark telemetry and pilot telemetry in the same database
- prefer a single-operator boundary over shared storage for session files and persistent browser contexts

## 7. Notes

- this package covers self-hosted pilot operations, not a managed control plane
- RBAC, quota management, and tenant isolation are not part of this package
- dedicated retention enforcement and audit export APIs are not included
- the compose artifact is an example for stdio pilot operation, not a guarantee for multi-operator orchestration
