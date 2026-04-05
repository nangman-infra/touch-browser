import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/real-user-research-benchmark/report.json`;

describe("real user research benchmark", () => {
  it("proves public multi-tab research tasks in a user-like MCP workflow", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly scenarioCount: number;
      readonly passedScenarioCount: number;
      readonly totalExtractedClaimCount: number;
      readonly totalSupportedClaimCount: number;
      readonly averageSupportedClaimRate: number;
      readonly averageListedTabCount: number;
      readonly uniqueDomainCount: number;
      readonly scenarios: ReadonlyArray<{
        readonly status: string;
        readonly taskProof: {
          readonly extractedClaimCount: number;
          readonly supportedClaimRate: number;
          readonly listedTabCount: number;
          readonly closed: boolean;
        };
      }>;
    }>(reportPath);

    expect(report.status).toBe("real-user-validated");
    expect(report.scenarioCount).toBeGreaterThanOrEqual(3);
    expect(report.passedScenarioCount).toBe(report.scenarioCount);
    expect(report.totalExtractedClaimCount).toBeGreaterThanOrEqual(8);
    expect(report.totalSupportedClaimCount).toBe(
      report.totalExtractedClaimCount,
    );
    expect(report.averageSupportedClaimRate).toBe(1);
    expect(report.averageListedTabCount).toBeGreaterThanOrEqual(2);
    expect(report.uniqueDomainCount).toBeGreaterThanOrEqual(4);
    expect(
      report.scenarios.every(
        (scenario) =>
          scenario.status === "passed" &&
          scenario.taskProof.extractedClaimCount >= 2 &&
          scenario.taskProof.supportedClaimRate >= 1 &&
          scenario.taskProof.listedTabCount >= 2 &&
          scenario.taskProof.closed === true,
      ),
    ).toBe(true);
  });
});
