# CLI Surface Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `touch-browser binary commands and JSON outputs`

## 1. Overview

This document fixes the current `touch-browser` CLI surface.

Included scope:

- stable research surface
- experimental supervised surface
- read-only fixture browsing
- live URL open, extract, and policy
- Playwright browser-backed open, extract, and policy
- persisted browser session commands
- allowlisted interactive browser session commands
- compact snapshot commands
- session synthesis
- browser replay
- stdio JSON-RPC serve mode
- long-lived daemon session registry
- multi-tab daemon orchestration
- replay and memory summary utilities
- JSON-only stdout contracts

## 2. Commands

Stable research surface:

| Command | Description |
| --- | --- |
| `touch-browser open <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Open a target and compile a structured snapshot. |
| `touch-browser snapshot <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return the full snapshot payload for the target. |
| `touch-browser compact-view <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return compact semantic text plus the `refIndex`. |
| `touch-browser extract <target> --claim <statement> ... [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return supported and unsupported evidence with citation metadata. |
| `touch-browser policy <target> [--browser] [--headed] [--budget <tokens>] [--allow-domain <host> ...]` | Return the allow, review, or block policy report. |
| `touch-browser session-snapshot --session-file <path>` | Read the latest snapshot from a persisted browser session. |
| `touch-browser session-compact --session-file <path>` | Return the compact semantic view for a persisted browser session. |
| `touch-browser session-extract --session-file <path> --claim <statement> ...` | Run evidence extraction against a persisted browser session. |
| `touch-browser session-synthesize --session-file <path> [--note-limit <count>]` | Combine a multi-page session into structured notes, claims, and citations. |
| `touch-browser follow --session-file <path> --ref <stable-ref> [--headed]` | Continue a persisted browser session by following a stable ref. |
| `touch-browser paginate --session-file <path> --direction next|prev [--headed]` | Paginate inside a persisted browser session. |
| `touch-browser expand --session-file <path> --ref <stable-ref> [--headed]` | Expand a target block inside a persisted browser session. |
| `touch-browser browser-replay --session-file <path>` | Reconstruct the persisted browser session from the replay perspective. |
| `touch-browser session-close --session-file <path>` | Close a persisted browser session and clean up its browser context. |
| `touch-browser telemetry-summary` | Return the aggregate pilot telemetry summary. |
| `touch-browser telemetry-recent [--limit <count>]` | Return recent telemetry events. |
| `touch-browser replay <scenario-name>` | Replay a recorded scenario transcript. |
| `touch-browser memory-summary [--steps <even-number>]` | Generate a long-session memory compaction summary. |
| `touch-browser serve` | Start the stdio JSON-RPC daemon. |

Experimental supervised surface:

| Command | Description |
| --- | --- |
| `touch-browser checkpoint --session-file <path>` | Return the current supervised browser risk state, provider hint, and approval guidance. |
| `touch-browser session-policy --session-file <path>` | Read the policy report for a persisted browser session. |
| `touch-browser session-profile --session-file <path>` | Read the active supervised policy profile. |
| `touch-browser set-profile --session-file <path> --profile research-read-only|research-restricted|interactive-review|interactive-supervised-auth|interactive-supervised-write` | Set the supervised policy profile. |
| `touch-browser approve --session-file <path> --risk challenge|mfa|auth|high-risk-write [--risk ...]` | Record required risk acknowledgements on the session. |
| `touch-browser click --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]` | Execute an interactive click inside a supervised session. |
| `touch-browser type --session-file <path> --ref <stable-ref> --value <text> [--headed] [--sensitive] [--ack-risk challenge|mfa|auth|high-risk-write ...]` | Type into a field inside a supervised session. |
| `touch-browser submit --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]` | Submit a form or control inside a supervised session. |
| `touch-browser refresh --session-file <path> [--headed]` | Recompile a supervised session after interaction. |

These are product-boundary categories. The implementation still lives in the same binary.

## 3. Execution Paths

- fixture targets run through `ReadOnlyActionVm`
- live targets run through `ReadOnlyRuntime + AcquisitionEngine + PolicyKernel`
- browser targets run through `Playwright stdio adapter -> ObservationCompiler -> ReadOnlyRuntime.open_snapshot -> Policy/Evidence`
- persisted browser sessions run through `session-file JSON -> ReadOnlySession + persisted browser state + browser context dir + browser trace + requested budget restore -> stable-ref hints -> Playwright action -> runtime append -> session-file save`
- browser interactive actions require allowlist, policy preflight, and explicit sensitive-field opt-in where relevant
- supervised browser actions additionally require `ack-risk`, and live sites currently require `--headed`
- `checkpoint` returns provider hints, required acknowledgement risks, active and recommended policy profiles, an approval panel, a provider playbook, and candidate controls
- `approve` stores approved risks in the session file so repeated acknowledgements do not need to be passed on every command and can also promote the policy profile for supervised auth or write paths
- `session-profile` and `set-profile` read and write the persisted browser session policy profile directly
- `telemetry-summary` and `telemetry-recent` query the pilot telemetry SQLite store directly
- commands sharing the same `--session-file` are serialized through the persistent context lock

Live and browser paths currently support:

- `open`
- `extract`
- `policy`
- `follow`
- `click`
- `type`
- `submit`
- `paginate`
- `expand`

Serve daemon methods:

- `runtime.session.create`
- `runtime.session.open`
- `runtime.session.snapshot`
- `runtime.session.compactView`
- `runtime.session.extract`
- `runtime.session.policy`
- `runtime.session.synthesize`
- `runtime.session.follow`
- `runtime.session.click`
- `runtime.session.type`
- `runtime.session.typeSecret`
- `runtime.session.submit`
- `runtime.session.refresh`
- `runtime.session.checkpoint`
- `runtime.session.approve`
- `runtime.session.profile.get`
- `runtime.session.profile.set`
- `runtime.session.secret.store`
- `runtime.session.secret.clear`
- `runtime.telemetry.summary`
- `runtime.telemetry.recent`
- `runtime.session.paginate`
- `runtime.session.expand`
- `runtime.session.replay`
- `runtime.session.close`
- `runtime.tab.open`
- `runtime.tab.list`
- `runtime.tab.select`
- `runtime.tab.close`

## 4. Output Contract

- stdout is always JSON
- the CLI does not emit human-friendly prose on stdout
- failures use stderr plus a non-zero exit code

Primary output shapes:

- `open` and `snapshot` -> `ActionResult`
- `compact-view` and `session-compact` -> compact snapshot payload plus `refIndex`
- `extract` -> `open` + `extract` + `sessionState`
- `policy` -> `policy` + `sessionState`
- `session-synthesize` -> `report` + `sessionState` + `sessionFile`
- `browser-replay` -> `replayedActions` + `compactText` + `sessionState`
- `replay` -> `sessionState` + `replayTranscript` + counts
- `memory-summary` -> `requestedActions` + `actionCount` + `sessionState` + `memorySummary`
- `serve` -> line-delimited JSON-RPC responses only

Interactive action outputs such as `click`, `type`, `submit`, and `refresh` return:

- `action`
- `policy`
- `sessionState`
- `result`

`result` is a compatibility alias. Consumers should interpret the action output through `action`.

`checkpoint` returns `checkpoint`, `policy`, `result`, and `sessionState`, including:

- `checkpoint.approvalPanel`
- `checkpoint.playbook`
- `checkpoint.activePolicyProfile`
- `checkpoint.recommendedPolicyProfile`

`approve` returns:

- `approvedRisks`
- `policyProfile`
- `result`
- `sessionState`
- `sessionFile`

`telemetry-summary` returns the aggregate summary.
`telemetry-recent` returns the recent event list.

## 5. Validation

Rust tests cover:

- fixture open CLI
- hostile policy CLI
- browser-backed fixture open CLI
- browser-backed extract CLI
- browser-backed hostile policy CLI
- browser session snapshot persistence
- browser session paginate
- browser session double-paginate DOM persistence
- browser session follow and session-extract
- browser session duplicate-follow stable-ref ordinal behavior
- browser session requested-budget persistence
- browser session expand and session-extract
- browser session interactive type
- browser session sensitive input rejection
- browser session interactive click
- browser session interactive submit
- browser session supervised MFA submit
- browser session supervised high-risk submit
- browser session refresh
- browser session checkpoint and approve persistence
- session profile parse and set behavior
- telemetry store and summary behavior
- compact-view CLI
- session-compact CLI
- browser-replay CLI
- session-close browser context cleanup
- replay CLI
- 50-action memory CLI

Direct execution validation covers:

- fixture `open --browser`
- pricing `extract --browser`
- hostile `policy --browser`
- browser session `open -> follow -> session-extract -> session-close`
- browser session `open -> type -> click -> session-close`
- browser session `open -> type -> submit -> session-close`
- browser session `open -> sensitive type -> supervised submit -> refresh -> session-close`
- browser session `open -> checkpoint -> approve -> supervised auth type and submit -> refresh -> session-close`
- real-site `open -> checkpoint -> approve -> type/typeSecret -> submit -> refresh` GitHub auth smoke
- browser session `open -> paginate -> session-close`
- browser session `open -> expand -> session-extract -> session-close`
- browser session `open -> session-compact -> session-synthesize -> browser-replay -> session-close`
- stdio JSON-RPC `serve -> runtime.status -> runtime.open`
- stdio JSON-RPC `serve -> runtime.session.create -> runtime.session.open -> runtime.tab.open -> runtime.session.synthesize -> runtime.session.close`
- stdio JSON-RPC `serve -> runtime.session.secret.store -> runtime.session.typeSecret -> runtime.session.submit -> runtime.session.refresh`
- stdio JSON-RPC `serve -> runtime.session.profile.get|set -> runtime.telemetry.summary|recent`
- MCP bridge `initialize -> tools/list -> tools/call(tb_status)`

## 6. Notes

- compact text removes block IDs for token efficiency and keeps action refs in `refIndex`
- the serve daemon keeps long-lived session and tab registries, but each tab still uses its own persisted browser context and session file
- browser-backed `follow` is supported on persisted sessions, not as general live multi-step replay
- `--budget` controls the observation budget for live and browser open paths and is reused during follow, paginate, and expand recompilation inside a browser session
- interactive actions are only supported inside allowlisted browser sessions
- credential-like sensitive input requires `--sensitive`
- submit works against either a form stable ref or a submit-control stable ref
- non-sensitive typed values are reapplied in the same browser pass immediately before submit
- sensitive values stay redacted in output and replay
- direct CLI secret replay uses the secret sidecar next to the session file
- the serve daemon uses the in-memory secret store through `runtime.session.typeSecret` and `runtime.session.secret.store`
- CAPTCHA, MFA, sensitive auth, and high-risk write are supervised review-gated paths, not bypass paths
- interactive actions stop when those signals appear unless the required acknowledgement is present
- once `checkpoint -> approve` has run, later commands in the same session do not need to repeat the same acknowledgement flags
- supervised flows can return provider-specific playbooks and approval panels for GitHub, Google, Auth0, Okta, Microsoft, and generic auth or challenge flows
- supported policy profiles are `research-read-only`, `research-restricted`, `interactive-review`, `interactive-supervised-auth`, and `interactive-supervised-write`
- pilot telemetry is stored in `output/pilot/telemetry.sqlite` by default and can be overridden with `TOUCH_BROWSER_TELEMETRY_DB` and `TOUCH_BROWSER_TELEMETRY_SURFACE`
- live non-fixture supervised interaction currently requires `--headed`
- persistent contexts are protected by a cross-process lock and may wait or return a busy error if another command is already using the same session
- allowlists currently enforce both domain boundaries and ref preflight blocking
- per-subcommand JSON Schema is not yet split out even though the stdout contract is stable
