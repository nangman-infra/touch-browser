# CLI Surface Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `touch-browser binary commands, serve methods, and MCP-facing command contracts`

## 1. Overview

This document fixes the current public CLI surface for `touch-browser`.

The runtime now has two read surfaces:

- `search`: browser-first discovery surface that structures Google or Brave result pages into ranked candidate links and next-action hints
- `read-view`: readable Markdown for higher-level review or a verifier model
- `compact-view`: low-token semantic state for routing, planning, and large multi-page runs

Evidence extraction is intentionally phrased as support retrieval, not final truth judgment:

- `evidenceSupportedClaims`
- `contradictedClaims`
- `insufficientEvidenceClaims`
- `needsMoreBrowsingClaims`
- `claimOutcomes`
- `supportScore`
- optional `verification` from `--verifier-command`

## 2. Stable Research Surface

| Command | Description |
| --- | --- |
| `touch-browser search <query> [--engine google\|brave] [--headed] [--budget <tokens>] [--session-file <path>]` | Open a Google or Brave search results page inside the browser runtime and return `ready`, `challenge`, or `no-results` status plus structured result items and next-action hints. |
| `touch-browser search-open-result --session-file <path> --rank <number> [--headed]` | Open one saved search result from a persisted browser search session when the latest saved search is `ready`. |
| `touch-browser open <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Open a target and compile a structured snapshot. |
| `touch-browser snapshot <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return the full snapshot payload for the target. |
| `touch-browser read-view <target> [--browser] [--headed] [--main-only] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return a readable Markdown rendering of the target. By default the renderer prefers main-content blocks when available; `--main-only` makes that filter explicit. |
| `touch-browser compact-view <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return compact semantic text plus `refIndex`. |
| `touch-browser extract <target> --claim <statement> ... [--verifier-command <shell-command>] [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--allow-domain <host> ...]` | Return structured claim outcomes with citations: `evidence-supported`, `contradicted`, `insufficient-evidence`, or `needs-more-browsing`. |
| `touch-browser policy <target> [--browser] [--headed] [--budget <tokens>] [--allow-domain <host> ...]` | Return the allow, review, or block policy report. |
| `touch-browser session-snapshot --session-file <path>` | Read the latest snapshot from a persisted browser session. |
| `touch-browser session-read --session-file <path> [--main-only]` | Return a readable Markdown rendering of the latest persisted browser snapshot. |
| `touch-browser session-compact --session-file <path>` | Return the compact semantic view for a persisted browser session. |
| `touch-browser session-extract --session-file <path> --claim <statement> ... [--verifier-command <shell-command>]` | Run evidence extraction against a persisted browser session. |
| `touch-browser session-synthesize --session-file <path> [--note-limit <count>] [--format json|markdown]` | Combine a multi-page session into structured notes, claims, and citations, or emit a Markdown report. |
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

## 3. Experimental Supervised Surface

| Command | Description |
| --- | --- |
| `touch-browser checkpoint --session-file <path>` | Return the current supervised browser risk state, provider hint, and approval guidance. |
| `touch-browser session-policy --session-file <path>` | Read the policy report for a persisted browser session. |
| `touch-browser session-profile --session-file <path>` | Read the active supervised policy profile. |
| `touch-browser set-profile --session-file <path> --profile research-read-only\|research-restricted\|interactive-review\|interactive-supervised-auth\|interactive-supervised-write` | Set the supervised policy profile. |
| `touch-browser approve --session-file <path> --risk challenge\|mfa\|auth\|high-risk-write [--risk ...]` | Record required risk acknowledgements on the session. |
| `touch-browser click --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge\|mfa\|auth\|high-risk-write ...]` | Execute an interactive click inside a supervised session. |
| `touch-browser type --session-file <path> --ref <stable-ref> --value <text> [--headed] [--sensitive] [--ack-risk challenge\|mfa\|auth\|high-risk-write ...]` | Type into a field inside a supervised session. |
| `touch-browser submit --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge\|mfa\|auth\|high-risk-write ...]` | Submit a form or control inside a supervised session. |
| `touch-browser refresh --session-file <path> [--headed]` | Recompile a supervised session after interaction. |

## 4. Execution Paths

- fixture targets run through `ReadOnlyActionVm`
- live targets run through `ReadOnlyRuntime + AcquisitionEngine + PolicyKernel`
- browser targets run through `Playwright stdio adapter -> ObservationCompiler -> ReadOnlyRuntime.open_snapshot -> Policy/Evidence`
- the observation compiler now also summarizes JSON-LD and common hydration blobs so JS-heavy pages expose more than just visible DOM text
- search targets run through `query -> engine URL builder -> browser open -> semantic SERP structuring -> ranked result items + next-action hints`
- persisted browser sessions run through `session-file JSON -> ReadOnlySession + persisted browser state + browser context dir + browser trace + requested budget restore -> stable-ref hints -> Playwright action -> runtime append -> session-file save`
- verifier hooks run only when `--verifier-command` is set and execute after evidence retrieval, with the ability to adjudicate the final claim verdict
- `read-view` and `session-read` prefer main-content blocks by default and can be forced into explicit main-zone filtering with `--main-only`
- supervised browser actions require allowlists, policy preflight, and explicit risk acknowledgement when challenge, MFA, auth, or high-risk-write signals appear

## 5. Serve Methods

- `runtime.open`
- `runtime.readView`
- `runtime.extract`
- `runtime.policy`
- `runtime.compactView`
- `runtime.search`
- `runtime.search.openResult`
- `runtime.search.openTop`
- `runtime.session.create`
- `runtime.session.open`
- `runtime.session.snapshot`
- `runtime.session.readView`
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

## 6. Output Contract

- most direct CLI commands emit JSON on stdout
- `read-view` and `session-read` emit raw Markdown in direct CLI mode
- `session-synthesize --format markdown` emits raw Markdown in direct CLI mode
- `serve` and MCP always return structured JSON
- failures use stderr plus a non-zero exit code

Primary shapes:

- `search` -> `search` + `result` + optional persisted `sessionFile`
- `open` and `snapshot` -> `ActionResult`
- `read-view` and `session-read` -> Markdown rendering plus metadata when consumed as JSON
- `compact-view` and `session-compact` -> compact snapshot payload plus `refIndex`
- `extract` -> `open` + `extract` + `sessionState`
- `policy` -> `policy` + `sessionState`
- `session-synthesize` -> `report` + `sessionState` + `sessionFile` + optional `markdown`
- `browser-replay` -> `replayedActions` + `compactText` + `sessionState`
- `replay` -> `sessionState` + `replayTranscript` + counts
- `memory-summary` -> `requestedActions` + `actionCount` + `sessionState` + `memorySummary`
- `serve` -> line-delimited JSON-RPC responses only

Evidence output terminology:

- `evidenceSupportedClaims`: retrieved evidence blocks that currently support a claim
- `contradictedClaims`: retrieved evidence that directly conflicts with the claim
- `insufficientEvidenceClaims`: claims with no sufficient support in the current snapshot
- `needsMoreBrowsingClaims`: claims that should stay unresolved until the agent opens a more specific source
- `claimOutcomes`: the canonical four-state verdict list across all extracted claims
- `supportScore`: evidence-match score for the retrieved support
- `verification`: optional second-pass verifier output supplied by `--verifier-command`, which may refine the final verdict while leaving the collected support trace intact
- search output includes `results`, `recommendedResultRanks`, and `nextActionHints` so a higher-level AI can decide the next browsing step without pretending the browser already knows the final answer
- each `nextActionHint` also includes `actor`, `canAutoRun`, and `headedRequired` so touch-browser can separate AI-owned follow-up from human checkpoints
- search output also includes `status` and optional `statusDetail` so challenge pages and empty result pages are explicit instead of masquerading as normal zero-result searches

## 7. Validation

Rust tests cover:

- search CLI parsing
- search result structuring
- HTML-based search result recovery when snapshot blocks are sparse
- search challenge detection for provider verification pages
- fixture open CLI
- hostile policy CLI
- read-view CLI
- extract verifier hook
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
- session-synthesize Markdown format
- telemetry store and summary behavior
- compact-view CLI
- session-compact CLI
- browser-replay CLI
- session-close browser context cleanup
- replay CLI
- 50-action memory CLI

Eval and smoke validation covers:

- contract schema validation for evidence and session synthesis payloads
- MCP bridge smoke
- serve and MCP interface compatibility
- session synthesis artifacts
- reference workflow artifacts
- example verifier hook at [scripts/example-verifier.mjs](../scripts/example-verifier.mjs)

## 8. Notes

- `read-view` is for readable inspection; `compact-view` is for low-token agent loops
- `search` is the discovery surface; it structures result pages, but it does not replace the later `read-view` / `extract` pass on the selected tabs
- use `--main-only` when the page shell is noisy and you want the Markdown output scoped to the primary content region
- verifier hooks do not replace the base extractor; they adjudicate the final claim verdict on top of the same collected support trace
- browser-backed `follow` is supported on persisted sessions, not as a general live multi-step replay
- `--budget` controls the observation budget for live and browser open paths and is reused during follow, paginate, and expand recompilation
- interactive actions are only supported inside allowlisted browser sessions
- credential-like sensitive input requires `--sensitive` or the daemon secret store
- CAPTCHA, MFA, sensitive auth, and high-risk write remain supervised review-gated paths
- pilot telemetry is stored in `output/pilot/telemetry.sqlite` by default and can be overridden with `TOUCH_BROWSER_TELEMETRY_DB` and `TOUCH_BROWSER_TELEMETRY_SURFACE`
