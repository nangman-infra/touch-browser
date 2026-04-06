import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/latency-cost-metrics/report.json`;

describe("latency and cost metrics", () => {
  it("records positive command timings and a compact token-cost advantage", async () => {
    const report = await readJsonFile<{
      readonly fixtureCompactMs: number;
      readonly liveOpenMs: number;
      readonly browserOpenMs: number;
      readonly liveExtractMs: number;
      readonly compactTokenCostRatio: number;
      readonly browserLatencyMultiplier: number;
    }>(reportPath);

    expect(report.fixtureCompactMs).toBeGreaterThan(0);
    expect(report.liveOpenMs).toBeGreaterThan(0);
    expect(report.browserOpenMs).toBeGreaterThan(0);
    expect(report.liveExtractMs).toBeGreaterThan(0);
    expect(report.compactTokenCostRatio).toBeLessThan(1);
    expect(report.browserLatencyMultiplier).toBeGreaterThan(1);
  });
});
