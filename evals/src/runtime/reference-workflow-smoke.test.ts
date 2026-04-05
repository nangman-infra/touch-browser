import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/reference-research-workflow/report.json`;

describe("reference workflow smoke", () => {
  it("produces a sample MCP-backed research workflow artifact", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly tools: string[];
      readonly sessionId: string;
      readonly pricingExtract: {
        readonly result: {
          readonly extract: {
            readonly output: {
              readonly supportedClaims: Array<{ readonly statement: string }>;
            };
          };
        };
      };
      readonly synthesis: {
        readonly activeTabId: string;
        readonly report: {
          readonly snapshotCount: number;
          readonly visitedUrls: string[];
        };
      };
      readonly closed: {
        readonly removed: boolean;
      };
    }>(reportPath);

    expect(report.status).toBe("ok");
    expect(report.tools).toContain("tb_open");
    expect(report.tools).toContain("tb_session_synthesize");
    expect(report.sessionId).toMatch(/^srvsess-/);
    expect(
      report.pricingExtract.result.extract.output.supportedClaims[0]?.statement,
    ).toBe("The Starter plan costs $29 per month.");
    expect(report.synthesis.activeTabId).toMatch(/^tab-/);
    expect(report.synthesis.report.snapshotCount).toBeGreaterThanOrEqual(2);
    expect(report.synthesis.report.visitedUrls.length).toBeGreaterThanOrEqual(
      2,
    );
    expect(report.closed.removed).toBe(true);
  });
});
