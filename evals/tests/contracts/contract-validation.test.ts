import { copyFile, mkdir, mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { generateMcpToolCatalog } from "../../../scripts/generate-mcp-tool-catalog.mjs";
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

    expect(registry.schemas.size).toBe(15);
    expect([...registry.schemas.keys()]).toEqual([
      "acquisition-record.schema.json",
      "action-command.schema.json",
      "action-result.schema.json",
      "evidence-block.schema.json",
      "evidence-report.schema.json",
      "json-rpc-request.schema.json",
      "json-rpc-response.schema.json",
      "mcp-tool-catalog.schema.json",
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
      "mcp-tool-catalog.schema.json",
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

  it("keeps the generated MCP tool catalog in sync with the canonical schema", async () => {
    const registry = await loadContractSchemas();
    const generatedCatalogPath = path.join(
      path.dirname(contractsManifestPath),
      "mcp-tool-catalog.json",
    );
    const generatedCatalog = await readJsonFile(generatedCatalogPath);

    expect(
      requireValidator(
        registry,
        "mcp-tool-catalog.schema.json",
      )(generatedCatalog),
    ).toBe(true);
  });

  it("keeps generated MCP output schemas compatible with hosted inspectors", async () => {
    const generatedCatalogPath = path.join(
      path.dirname(contractsManifestPath),
      "mcp-tool-catalog.json",
    );
    const generatedCatalog = await readJsonFile(generatedCatalogPath);

    expect(Array.isArray(generatedCatalog)).toBe(true);
    for (const tool of generatedCatalog as Array<{
      readonly name: string;
      readonly outputSchema: Record<string, unknown>;
    }>) {
      expect(tool.outputSchema.type).toBe("object");
      expect(tool.outputSchema).not.toHaveProperty("anyOf");
      expect(tool.outputSchema).not.toHaveProperty("oneOf");
      expect(tool.outputSchema).not.toHaveProperty("allOf");
    }
  });

  it("keeps the checked-in MCP tool catalog generated from the current source", async () => {
    const tempRoot = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-mcp-tool-catalog-"),
    );

    try {
      const tempSchemaDir = path.join(tempRoot, "contracts", "schemas");
      await mkdir(tempSchemaDir, { recursive: true });
      await copyFile(
        path.join(contractsDir, "mcp-tool-catalog.schema.json"),
        path.join(tempSchemaDir, "mcp-tool-catalog.schema.json"),
      );

      await generateMcpToolCatalog(tempRoot);

      const checkedInCatalog = JSON.parse(
        await readFile(
          path.join(
            path.dirname(contractsManifestPath),
            "mcp-tool-catalog.json",
          ),
          "utf8",
        ),
      );
      const regeneratedCatalog = JSON.parse(
        await readFile(
          path.join(
            tempRoot,
            "contracts",
            "generated",
            "mcp-tool-catalog.json",
          ),
          "utf8",
        ),
      );

      expect(regeneratedCatalog).toEqual(checkedInCatalog);
    } finally {
      await rm(tempRoot, { recursive: true, force: true });
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
        supportScore: 0.94,
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
        evidenceSupportedClaims: [
          {
            version: "1.0.0",
            claimId: "c1",
            statement: "The Starter plan costs $29 per month.",
            support: ["b3", "b4"],
            supportScore: 0.91,
            citation: {
              url: "fixture://research/citation-heavy/pricing",
              retrievedAt: "2026-03-14T12:00:00+09:00",
              sourceType: "fixture",
              sourceRisk: "low",
              sourceLabel: "Pricing",
            },
          },
        ],
        contradictedClaims: [
          {
            version: "1.0.0",
            claimId: "c3",
            statement: "The Starter plan costs $99 per day.",
            reason: "numeric-mismatch",
            checkedBlockRefs: ["rmain:table:pricing"],
          },
        ],
        insufficientEvidenceClaims: [
          {
            version: "1.0.0",
            claimId: "c2",
            statement: "There is an Enterprise plan.",
            reason: "no-supporting-block",
          },
        ],
        needsMoreBrowsingClaims: [
          {
            version: "1.0.0",
            claimId: "c4",
            statement: "The plan is available in all regions.",
            reason: "needs-more-browsing",
            nextActionHint:
              "Browse the regional availability or feature-matrix page before answering.",
          },
        ],
        claimOutcomes: [
          {
            version: "1.0.0",
            claimId: "c1",
            statement: "The Starter plan costs $29 per month.",
            verdict: "evidence-supported",
            support: ["b3", "b4"],
            supportScore: 0.91,
            citation: {
              url: "fixture://research/citation-heavy/pricing",
              retrievedAt: "2026-03-14T12:00:00+09:00",
              sourceType: "fixture",
              sourceRisk: "low",
              sourceLabel: "Pricing",
            },
          },
          {
            version: "1.0.0",
            claimId: "c4",
            statement: "The plan is available in all regions.",
            verdict: "needs-more-browsing",
            reason: "needs-more-browsing",
            checkedBlockRefs: ["rmain:heading:pricing"],
          },
        ],
      }),
    ).toBe(true);

    expect(
      requireValidator(
        registry,
        "policy-report.schema.json",
      )({
        decision: "review",
        sourceRisk: "hostile",
        riskClass: "high",
        blockedRefs: ["rmain:link:https-malicious-example-submit"],
        signals: [
          {
            kind: "external-actionable",
            origin: "policy-boundary",
            stableRef: "rmain:link:https-malicious-example-submit",
            detail:
              "External actionable element is blocked on hostile sources.",
          },
        ],
        pageRisk: {
          decision: "review",
          riskClass: "high",
        },
        actionRisk: {
          decision: "block",
          riskClass: "blocked",
        },
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
          pageRisk: {
            decision: "allow",
            riskClass: "low",
          },
          actionRisk: {
            decision: "allow",
            riskClass: "low",
          },
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
      "mcp-tool-catalog.schema.json",
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
