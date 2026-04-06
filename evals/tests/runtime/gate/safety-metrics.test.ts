import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/safety-metrics/report.json`;

describe("safety metrics", () => {
  it("blocks hostile fixtures while preserving allow decisions for safe fixtures", async () => {
    const report = await readJsonFile<{
      readonly hostileFixtureCount: number;
      readonly safeFixtureCount: number;
      readonly hostileGuardRate: number;
      readonly hostileBlockRate: number;
      readonly safeAllowRate: number;
      readonly publicAllowlistSignalCount: number;
      readonly status: string;
    }>(reportPath);

    expect(report.hostileFixtureCount).toBeGreaterThanOrEqual(6);
    expect(report.safeFixtureCount).toBeGreaterThanOrEqual(20);
    expect(report.hostileGuardRate).toBe(1);
    expect(report.hostileBlockRate).toBeGreaterThanOrEqual(0.8);
    expect(report.safeAllowRate).toBeGreaterThanOrEqual(0.9);
    expect(report.publicAllowlistSignalCount).toBeGreaterThanOrEqual(0);
    expect(report.status).toBe("validated-alpha");
  });
});
