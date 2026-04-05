# Doc Link Integrity Spec

- Status: `Active`
- Version: `v1`
- Last Updated: `2026-04-05`
- Scope: `tracked markdown link validation for the public repository`

## 1. Overview

This document exists to answer a simple product question:

- do the links in the public repository actually resolve right now

The check is not advisory prose. It is backed by a generated report:

- [report.json](../fixtures/scenarios/doc-link-integrity/report.json)

Runner:

- `pnpm run fixtures:doc-links`

## 2. What Gets Checked

The integrity pass walks tracked Markdown files and validates:

- relative file links
- relative heading anchors for Markdown targets
- external `https://` documentation links used in tracked Markdown

## 3. Why This Matters

If public docs link to missing files or dead URLs, benchmark claims lose credibility before the reader evaluates the product itself.

This check therefore acts as a documentation trust gate, not just a formatting check.

## 4. Current Interpretation

Current generated baseline on `2026-04-05`:

- tracked Markdown files checked: `18`
- relative file failures: `0`
- local heading-anchor failures: `0`
- external documentation links checked live: `17`
- external documentation link failures: `0`

When the report status is `ok`, the public docs pass three conditions:

- no missing relative file targets
- no broken local heading anchors
- no failing external documentation links among the checked set
