import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/release-readiness/report.json`;

describe("release readiness", () => {
  it("tracks pilot readiness against internal quality, safety, and operations gates", async () => {
    const report = await readJsonFile<{
      readonly readinessScore: number;
      readonly status: string;
      readonly checks: {
        readonly coreReady: boolean;
        readonly longSessionReady: boolean;
        readonly mixedSourceReady: boolean;
        readonly observationReady: boolean;
        readonly operationsPackageReady: boolean;
        readonly daemonReady: boolean;
        readonly docsReady: boolean;
        readonly scriptsReady: boolean;
        readonly publicProofReady: boolean;
        readonly realUserEnvironmentReady: boolean;
        readonly compactTokenCostRatio: number;
      };
    }>(reportPath);

    expect(report.readinessScore).toBeGreaterThanOrEqual(0.7);
    expect(["pilot-ready", "alpha-ready"]).toContain(report.status);
    expect(report.checks.coreReady).toBe(true);
    expect(report.checks.longSessionReady).toBe(true);
    expect(report.checks.mixedSourceReady).toBe(true);
    expect(report.checks.observationReady).toBe(true);
    expect(report.checks.operationsPackageReady).toBe(true);
    expect(report.checks.daemonReady).toBe(true);
    expect(report.checks.docsReady).toBe(true);
    expect(report.checks.scriptsReady).toBe(true);
    expect(report.checks.publicProofReady).toBe(true);
    expect(report.checks.realUserEnvironmentReady).toBe(true);
    expect(report.checks.compactTokenCostRatio).toBeLessThan(0.7);
  });
});
