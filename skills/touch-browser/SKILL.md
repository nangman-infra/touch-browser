---
name: touch-browser
description: Use for evidence-first public web research and claim verification with touch-browser. Trigger when the task needs browser-backed discovery, page-grounded citations, claim extraction, or safe unresolved outcomes instead of guessed answers. Do not use for private account automation, uncontrolled browsing, or final truth claims beyond page-local evidence.
---

# Touch Browser

Use this skill when the task needs `touch-browser` for public documentation lookup, public web research, or page-grounded claim verification.

Read this file first. Use the reference files only when you need lower-level contract details.

## Use When

- the user needs browser-backed search over public docs or public research pages
- the task needs citations or support snippets tied to the current page
- the task needs `search -> open -> read -> extract`
- the task needs a safe unresolved outcome instead of a guessed answer
- the task needs multi-page research with replayable session state

## Do Not Use When

- the task is private account automation by default
- the task is checkout, purchase, banking, or account settings work
- the task only needs local code edits or repository analysis
- a plain static docs fetch is already sufficient
- the request asks for final truth claims beyond what the current page evidence supports

## Core Boundary

`touch-browser` is an evidence-first browsing and extraction layer.

It does:

- search and open public pages
- compile readable and compact page views
- extract page-local support, contradiction, or unresolved evidence
- preserve session state for multi-page work

It does not:

- act as a universal truth oracle
- justify certainty without evidence
- silently cross into auth, MFA, or high-risk write workflows

## Default Workflow

Choose the workflow based on whether the user already has a target page.

### Direct Page Workflow

Use this when the user already provides a URL or a specific page.

1. Open the page with `open` or inspect it with `read-view`.
2. Confirm the page is specific enough for the claim.
3. Run `extract`.
4. If several related pages were opened, use `session-synthesize` after evidence collection.

### Search-First Workflow

Use this when the user gives a topic or question instead of a final page.

1. Run `search`.
2. Inspect ranked results and `nextActionHints`.
3. Open a specific result with `search-open-result` or several with `search-open-top`.
4. Inspect `session-read` or `session-compact` before extracting.
5. Run `session-extract`.
6. Use `session-synthesize` only after page-level evidence has been collected.

## Preferred Surface Order

Prefer surfaces in this order:

1. `search` for discovery
2. `open`, `read-view`, or `compact-view` for scope checking
3. `extract` or `session-extract` for evidence retrieval
4. `session-synthesize` for multi-page reporting

Avoid synthesizing before evidence retrieval.

## Stop And Handoff Rules

Stop and hand off to a human when:

- `status` is `challenge`
- `nextActionHints` says recovery is human-owned
- auth or MFA is required
- the action would cross into high-risk write behavior

Browse more instead of answering when:

- the verdict is `needs-more-browsing`
- the verdict is `insufficient-evidence`
- the page is still too broad
- the current tab is a hub page instead of a specific source

Escalate to review instead of flattening the result when:

- the claim is contradicted
- different pages materially disagree
- `reviewRecommended` is true
- the confidence is not strong enough for reuse

## Output Contract

When you use this skill, the response should preserve:

1. the page or session used
2. the checked claim
3. the extractor verdict
4. the support snippet or citation when available
5. whether more browsing or review is needed

Do not hide unresolved states behind a simple yes/no answer.

## Verification Rules

Before reusing evidence:

1. inspect scope with `read-view` or `compact-view`
2. run `extract`
3. inspect `confidenceBand`, `reviewRecommended`, and the support snippets
4. browse more if the result is weak or unresolved

If the workflow requires a second-pass adjudicator, use the documented verifier path instead of increasing confidence by assertion alone.

## Commands In Scope

Primary commands for this skill:

- `search`
- `search-open-result`
- `search-open-top`
- `open`
- `read-view`
- `compact-view`
- `extract`
- `session-read`
- `session-compact`
- `session-extract`
- `session-synthesize`
- `session-close`

Commands out of scope by default:

- `update`
- `uninstall`
- telemetry maintenance commands unless explicitly requested
- supervised interaction commands such as `click`, `type`, `submit`, `approve`, `set-profile`

## References

Read these only when needed:

- [references/cli-surface.md](references/cli-surface.md): command surface and workflow grouping
- [references/mcp-bridge.md](references/mcp-bridge.md): MCP narrowing rules and headless boundary
- [references/evidence-rules.md](references/evidence-rules.md): evidence semantics and unresolved-state handling

## Examples

- [examples/search-first.md](examples/search-first.md)
- [examples/direct-page.md](examples/direct-page.md)
- [examples/unresolved-claim.md](examples/unresolved-claim.md)
