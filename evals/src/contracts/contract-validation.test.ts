import path from "node:path";

import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { contractsDir, contractsManifestPath } from "../support/paths.js";
import {
  loadContractSchemas,
  readSchemaSource,
  requireValidator,
} from "./schema-loader.js";

describe("contract schemas", () => {
  it("loads and compiles the initial schema set", async () => {
    const registry = await loadContractSchemas();

    expect(registry.schemas.size).toBe(14);
    expect([...registry.schemas.keys()]).toEqual([
      "acquisition-record.schema.json",
      "action-command.schema.json",
      "action-result.schema.json",
      "evidence-block.schema.json",
      "evidence-report.schema.json",
      "json-rpc-request.schema.json",
      "json-rpc-response.schema.json",
      "policy-report.schema.json",
      "replay-transcript.schema.json",
      "session-state.schema.json",
      "session-synthesis-report.schema.json",
      "snapshot-block.schema.json",
      "snapshot-document.schema.json",
      "unsupported-claim.schema.json",
    ]);
  });

  it("declares valid json schema documents", async () => {
    const schemaPaths = [
      "acquisition-record.schema.json",
      "snapshot-block.schema.json",
      "action-result.schema.json",
      "evidence-block.schema.json",
      "evidence-report.schema.json",
      "unsupported-claim.schema.json",
      "action-command.schema.json",
      "policy-report.schema.json",
      "session-state.schema.json",
      "session-synthesis-report.schema.json",
      "replay-transcript.schema.json",
      "json-rpc-request.schema.json",
      "json-rpc-response.schema.json",
      "snapshot-document.schema.json",
    ];

    for (const schemaFile of schemaPaths) {
      const schema = await readSchemaSource(
        path.join(contractsDir, schemaFile),
      );
      expect(schema.$schema).toBe(
        "https://json-schema.org/draft/2020-12/schema",
      );
      expect(schema.$id).toBe(schemaFile);
    }
  });

  it("accepts valid example payloads", async () => {
    const registry = await loadContractSchemas();

    expect(
      requireValidator(
        registry,
        "acquisition-record.schema.json",
      )({
        version: "1.0.0",
        requestedUrl: "https://example.com/docs#getting-started",
        finalUrl: "https://example.com/docs",
        sourceType: "http",
        statusCode: 200,
        contentType: "text/html; charset=utf-8",
        redirectChain: [
          "https://example.com/start",
          "https://example.com/docs",
        ],
        cacheStatus: "miss",
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "snapshot-document.schema.json",
      )({
        version: "1.0.0",
        stableRefVersion: "1",
        source: {
          sourceUrl: "fixture://research/static-docs/getting-started",
          sourceType: "fixture",
          title: "Touch Browser Docs - Getting Started",
        },
        budget: {
          requestedTokens: 512,
          estimatedTokens: 59,
          emittedTokens: 59,
          truncated: false,
        },
        blocks: [
          {
            version: "1.0.0",
            id: "b1",
            kind: "metadata",
            ref: "rhead:metadata:touch-browser-docs-getting-started",
            role: "metadata",
            text: "Touch Browser Docs - Getting Started",
            attributes: {
              source: "title",
              tagName: "title",
              textLength: 36,
              zone: "head",
            },
            evidence: {
              sourceUrl: "fixture://research/static-docs/getting-started",
              sourceType: "fixture",
              domPathHint: "html > head",
            },
          },
        ],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "snapshot-block.schema.json",
      )({
        version: "1.0.0",
        id: "b17",
        kind: "link",
        ref: "rnav:docs",
        role: "primary-nav",
        text: "Docs",
        attributes: {
          href: "/docs",
          actionable: true,
        },
        evidence: {
          sourceUrl: "fixture://research/static-docs/getting-started",
          sourceType: "fixture",
          domPathHint: "nav > a:nth-child(1)",
        },
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "evidence-block.schema.json",
      )({
        version: "1.0.0",
        claimId: "c12",
        statement: "The page offers monthly plans.",
        support: ["b21", "b22"],
        confidence: 0.94,
        citation: {
          url: "fixture://research/citation-heavy/pricing",
          retrievedAt: "2026-03-14T12:00:00+09:00",
          sourceType: "fixture",
          sourceRisk: "low",
          sourceLabel: "pricing fixture",
        },
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "unsupported-claim.schema.json",
      )({
        version: "1.0.0",
        claimId: "c99",
        statement: "The page contains an Enterprise plan.",
        reason: "no-supporting-block",
        checkedBlockRefs: ["rmain:heading:pricing"],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "evidence-report.schema.json",
      )({
        version: "1.0.0",
        generatedAt: "2026-03-14T12:00:00+09:00",
        source: {
          sourceUrl: "fixture://research/citation-heavy/pricing",
          sourceType: "fixture",
          sourceRisk: "low",
          sourceLabel: "Pricing",
        },
        supportedClaims: [
          {
            version: "1.0.0",
            claimId: "c1",
            statement: "The Starter plan costs $29 per month.",
            support: ["b3", "b4"],
            confidence: 0.91,
            citation: {
              url: "fixture://research/citation-heavy/pricing",
              retrievedAt: "2026-03-14T12:00:00+09:00",
              sourceType: "fixture",
              sourceRisk: "low",
              sourceLabel: "Pricing",
            },
          },
        ],
        unsupportedClaims: [
          {
            version: "1.0.0",
            claimId: "c2",
            statement: "There is an Enterprise plan.",
            reason: "no-supporting-block",
          },
        ],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "policy-report.schema.json",
      )({
        decision: "block",
        sourceRisk: "hostile",
        riskClass: "blocked",
        blockedRefs: ["rmain:link:https-malicious-example-submit"],
        signals: [
          {
            kind: "external-actionable",
            stableRef: "rmain:link:https-malicious-example-submit",
            detail:
              "External actionable element is blocked on hostile sources.",
          },
        ],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "action-command.schema.json",
      )({
        version: "1.0.0",
        action: "open",
        targetUrl: "https://example.com/docs",
        riskClass: "low",
        reason: "Open the docs landing page.",
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "action-result.schema.json",
      )({
        version: "1.0.0",
        action: "open",
        status: "succeeded",
        payloadType: "snapshot-document",
        output: {
          source: {
            sourceUrl: "fixture://research/static-docs/getting-started",
          },
        },
        policy: {
          decision: "allow",
          sourceRisk: "low",
          riskClass: "low",
          blockedRefs: [],
          signals: [],
        },
        message: "Opened document.",
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "session-state.schema.json",
      )({
        version: "1.0.0",
        sessionId: "sresearch001",
        mode: "read-only",
        status: "active",
        policyProfile: "research-read-only",
        currentUrl: "fixture://research/static-docs/getting-started",
        openedAt: "2026-03-14T12:00:00+09:00",
        updatedAt: "2026-03-14T12:00:05+09:00",
        visitedUrls: ["fixture://research/static-docs/getting-started"],
        snapshotIds: ["snap_docs_001"],
        workingSetRefs: ["rnav:docs", "rmain:h1"],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "replay-transcript.schema.json",
      )({
        version: "1.0.0",
        sessionId: "sresearch001",
        entries: [
          {
            seq: 1,
            timestamp: "2026-03-14T12:00:00+09:00",
            kind: "observation",
            payloadType: "acquisition-record",
            payload: {
              version: "1.0.0",
              requestedUrl: "https://example.com/docs#getting-started",
              finalUrl: "https://example.com/docs",
              sourceType: "http",
              statusCode: 200,
              contentType: "text/html; charset=utf-8",
              redirectChain: ["https://example.com/docs"],
              cacheStatus: "miss",
            },
          },
          {
            seq: 2,
            timestamp: "2026-03-14T12:00:00+09:00",
            kind: "command",
            payloadType: "action-command",
            payload: {
              action: "open",
              targetUrl: "fixture://research/static-docs/getting-started",
            },
          },
        ],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "json-rpc-request.schema.json",
      )({
        jsonrpc: "2.0",
        id: "req-1",
        method: "runtime.snapshot",
        params: {
          budget: 1200,
        },
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "json-rpc-response.schema.json",
      )({
        jsonrpc: "2.0",
        id: "req-1",
        result: {
          ok: true,
        },
      }),
    ).toBe(true);
  });

  it("rejects invalid payloads", async () => {
    const registry = await loadContractSchemas();

    const invalidSnapshot = requireValidator(
      registry,
      "snapshot-block.schema.json",
    );
    expect(
      invalidSnapshot({
        version: "1.0.0",
        id: "block-1",
        kind: "link",
        ref: "r1",
        role: "primary-nav",
        text: "Docs",
        attributes: {},
        evidence: {
          sourceUrl: "fixture://bad",
          sourceType: "fixture",
        },
      }),
    ).toBe(false);

    const invalidAction = requireValidator(
      registry,
      "action-command.schema.json",
    );
    expect(
      invalidAction({
        version: "1.0.0",
        action: "open",
        riskClass: "low",
        reason: "Missing target URL",
      }),
    ).toBe(false);

    const invalidResponse = requireValidator(
      registry,
      "json-rpc-response.schema.json",
    );
    expect(
      invalidResponse({
        jsonrpc: "2.0",
        id: "req-1",
      }),
    ).toBe(false);
  });

  it("matches the generated manifest", async () => {
    const manifest = await readJsonFile<{
      readonly schemas: readonly string[];
    }>(contractsManifestPath);

    expect(manifest.schemas).toEqual([
      "acquisition-record.schema.json",
      "action-command.schema.json",
      "action-result.schema.json",
      "evidence-block.schema.json",
      "evidence-report.schema.json",
      "json-rpc-request.schema.json",
      "json-rpc-response.schema.json",
      "policy-report.schema.json",
      "replay-transcript.schema.json",
      "session-state.schema.json",
      "session-synthesis-report.schema.json",
      "snapshot-block.schema.json",
      "snapshot-document.schema.json",
      "unsupported-claim.schema.json",
    ]);
  });
});
