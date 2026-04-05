import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/public-reference-workflow/report.json`;

describe("public reference workflow smoke", () => {
  it("produces a public-web MCP workflow artifact with supported claims and closed session cleanup", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly tools: string[];
      readonly sessionId: string;
      readonly openedTabs: ReadonlyArray<{
        readonly id: string;
        readonly target: string;
        readonly tabId: string;
      }>;
      readonly taskProof: {
        readonly extractedClaimCount: number;
        readonly supportedClaimCount: number;
        readonly supportedClaimRate: number;
        readonly synthesizedNoteCount: number;
      };
      readonly closed: {
        readonly removed: boolean;
      };
    }>(reportPath);

    expect(report.status).toBe("ok");
    expect(report.tools).toContain("tb_session_create");
    expect(report.tools).toContain("tb_tab_open");
    expect(report.tools).toContain("tb_session_synthesize");
    expect(report.sessionId).toMatch(/^srvsess-/);
    expect(report.openedTabs).toHaveLength(5);
    expect(report.taskProof.extractedClaimCount).toBe(4);
    expect(report.taskProof.supportedClaimCount).toBe(4);
    expect(report.taskProof.supportedClaimRate).toBe(1);
    expect(report.taskProof.synthesizedNoteCount).toBeGreaterThan(0);
    expect(report.closed.removed).toBe(true);
  }, 20_000);
});
