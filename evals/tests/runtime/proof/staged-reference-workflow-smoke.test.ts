import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/staged-reference-workflow/report.json`;

describe("staged reference workflow smoke", () => {
  it("produces a mixed public and trusted-source MCP workflow artifact", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly tools: string[];
      readonly sessionId: string;
      readonly phases: ReadonlyArray<{
        readonly id: string;
        readonly kind: string;
        readonly compactText?: string;
      }>;
      readonly sourceBoundary: {
        readonly publicTargets: string[];
        readonly trustedTargets: string[];
      };
      readonly taskProof: {
        readonly supportedClaimRate: number;
        readonly mixedSourceStageCount: number;
        readonly listedTabCount: number;
        readonly closedTabRemainingCount: number | null;
      };
      readonly closedSession: {
        readonly removed: boolean;
      };
    }>(reportPath);

    expect(report.status).toBe("ok");
    expect(report.tools).toContain("tb_tab_list");
    expect(report.tools).toContain("tb_tab_select");
    expect(report.tools).toContain("tb_tab_close");
    expect(report.sessionId).toMatch(/^srvsess-/);
    expect(report.phases).toHaveLength(2);
    expect(report.phases[1]?.compactText?.length ?? 0).toBeGreaterThan(0);
    expect(report.sourceBoundary.publicTargets).toHaveLength(1);
    expect(report.sourceBoundary.trustedTargets).toHaveLength(1);
    expect(report.taskProof.supportedClaimRate).toBe(1);
    expect(report.taskProof.mixedSourceStageCount).toBe(2);
    expect(report.taskProof.listedTabCount).toBeGreaterThanOrEqual(2);
    expect(report.taskProof.closedTabRemainingCount).toBe(1);
    expect(report.closedSession.removed).toBe(true);
  });
});
