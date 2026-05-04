# Portfolio Integration Spec

- Status: `Experimental`
- Version: `v1`
- Last Updated: `2026-05-05`
- Scope: `touch-connect handoff / A2A artifact mapping / evidence report media type`

## 1. Purpose

This document fixes the integration surface between `touch-browser` and external coordination layers.

`touch-browser` remains the evidence verifier. It does not become:

- an LLM answer generator
- an auth/SSO owner
- a multi-tenant SaaS control plane
- a cross-session persistent memory database

## 2. touch-connect Mapping

Recommended mapping:

| touch-connect concept | touch-browser field |
| --- | --- |
| `correlation_ref` | `tb_session_create.sessionId` |
| confidence band | `claimOutcomes[].confidenceBand` |
| evidence reference | `claimOutcomes[].claimId` + `primarySupportSnippet.stableRef` |
| citation reference | `claimOutcomes[].citation.url` + `retrievedAt` |
| review route | `reviewRecommended=true` or `confidenceBand=review` |
| verifier result | `verificationVerdict` + `verifierScore` |

Rules:

- caller-provided `sessionId` is the bridge hook for external task correlation
- touch-connect may wrap an evidence report, but should not rewrite evidence snippets
- approval and human handoff remain outside `touch-browser`

## 3. A2A Artifact Mapping

A2A treats task outputs as artifacts with typed parts. `EvidenceReport` maps cleanly to an artifact payload.

Recommended media type:

```text
application/vnd.touch-browser.evidence-report+json
```

Recommended artifact shape:

```json
{
  "name": "touch-browser-evidence-report",
  "description": "Page-grounded claim verification report with citations",
  "parts": [
    {
      "type": "application/vnd.touch-browser.evidence-report+json",
      "json": {
        "version": "1.1.0",
        "claimOutcomes": []
      }
    }
  ],
  "metadata": {
    "touchBrowserSessionId": "task:alpha_001",
    "evidenceReportVersion": "1.1.0"
  }
}
```

This is the adapter shape for a JSON-style A2A part. Use the A2A task/session layer for durable work identity. Use the artifact as the immutable evidence output.

## 4. Boundary

Do not put these into `touch-browser` before a separate contract exists:

- cloud SaaS tenancy
- SSO ownership
- cross-page final truth generation
- auto-retry agent loops
- long-lived database policy
