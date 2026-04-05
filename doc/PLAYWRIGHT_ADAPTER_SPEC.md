# Playwright Adapter Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `stdio JSON-RPC fallback adapter surface`

## 1. Overview

This document defines the current Playwright adapter implementation and the CLI handoff points that use it.

Included scope:

- stdio JSON-RPC request handling
- browser-backed `browser.snapshot`
- the `touch-browser --browser` handoff path
- adapter capability discovery
- low-risk and allowlisted interactive execution for `browser.follow`, `browser.click`, `browser.type`, `browser.submit`, `browser.paginate`, and `browser.expand`
- persistent browser context reuse through `contextDir`

Out of scope:

- screenshot, raw DOM, or accessibility capture as first-class outputs
- a standalone browser daemon separate from the current adapter process

## 2. Supported Methods

- `adapter.status`
- `browser.snapshot`
- `browser.follow`
- `browser.click`
- `browser.type`
- `browser.submit`
- `browser.paginate`
- `browser.expand`

## 3. Method Semantics

- `adapter.status`
  - returns adapter state and capability metadata
- `browser.snapshot`
  - launches Playwright Chromium and captures a `url`, `html`, or `contextDir` input
  - can reuse a persistent context from `contextDir`, which is how CLI `refresh` reloads the current page state
  - returns `finalUrl`, `title`, `visibleText`, `html`, `linkCount`, `buttonCount`, and `inputCount`
- `browser.paginate`
  - opens `url` or `html`, performs a next or previous pagination click, and returns updated `html`, `visibleText`, `finalUrl`, `clickedText`, and `page`
- `browser.follow`
  - opens `url` or `html`, clicks the matching anchor for the requested target text, href, or ref-derived hint set
  - stable refs from the CLI are translated into `targetText`, `targetHref`, `targetTagName`, `targetDomPathHint`, and `targetOrdinalHint`
  - falls back to safe same-origin navigation when a visible locator cannot be resolved but a safe internal href exists
  - returns updated `html`, `visibleText`, `finalUrl`, and `clickedText`
- `browser.click`
  - opens `url` or `html`, clicks the requested button, link, or control target, and returns updated `html`, `visibleText`, `finalUrl`, and `clickedText`
- `browser.type`
  - opens `url` or `html`, types into the requested input, textarea, or contenteditable target
  - uses the stable-ref hint set plus name, type, placeholder, value, and aria-label signals
  - never returns raw secret values for `sensitive` input
  - returns updated `html`, `visibleText`, `finalUrl`, and `typedLength`
- `browser.submit`
  - opens `url` or `html`, submits the requested form or submit control
  - can apply `prefill` values inside the same browser pass before submit
  - prefers `requestSubmit()` for form targets and falls back to a submit-control click otherwise
  - recollects the final DOM and URL after navigation races
  - returns updated `html`, `visibleText`, and `finalUrl`
- `browser.expand`
  - opens `url` or `html`, clicks the requested expandable button or link target
  - uses stable-ref hint translation to disambiguate duplicate controls
  - returns updated `html`, `visibleText`, `finalUrl`, and `clickedText`

## 4. Guarantees

- transport: `stdio-json-rpc`
- malformed requests are rejected with JSON-RPC errors
- browser-backed snapshots use headless Chromium by default
- the Rust CLI can call the adapter subprocess directly and feed the result into the semantic snapshot, evidence, and policy pipeline
- dynamic actions are exposed as `limitedDynamicAction: true`
- CLI-managed persisted sessions keep the current DOM, URL, and context directory so multi-command browser loops and browser replay can resume cleanly
- sessions that only retain a persistent context can still resync through `browser.snapshot`
- concurrent access to the same `contextDir` is serialized through a cross-process lock
- duplicate candidate selection uses stable-ref ordinal hints as well as DOM order
- hidden duplicate candidates are excluded through visible filtering
- some live-site navigation can recover through same-origin href fallback
- input locator scoring uses placeholder, name, type, value, and aria-label signals

## 5. Validation

Vitest:

- adapter status contract
- status request handling
- browser-backed snapshot request handling
- follow, paginate, and expand handling
- click, type, and submit handling
- duplicate link and button disambiguation
- safe href follow fallback handling
- malformed request rejection

Rust CLI:

- browser-backed fixture open
- browser-backed extract
- browser-backed hostile policy
- browser session follow and session-extract
- browser session duplicate-follow stable-ref ordinal behavior
- browser session paginate
- browser session double-paginate DOM persistence
- browser session expand and session-extract
- browser replay CLI
- browser context cleanup

## 6. Notes

- the adapter already completes the runtime observation handoff
- low-risk dynamic action supports browser-backed execution and persistent-context replay, but not a standalone daemon or native shared-context multi-tab
- interactive type, click, and submit remain restricted behind allowlists, policy preflight, and supervised risk acknowledgement
- live submit tries to preserve the most recent interactive DOM state instead of forcing a full reload first
- the adapter uses a session-directory lock to reduce profile lock conflicts across repeated direct CLI calls
- stable refs are translated into selector hints rather than mapped directly to native DOM node handles
- on large live sites, a visible actionable item in the snapshot may still diverge from the current viewport clickability
- anti-bot, CAPTCHA, and MFA are not bypass targets; the adapter’s role is to resync safely around those boundaries
