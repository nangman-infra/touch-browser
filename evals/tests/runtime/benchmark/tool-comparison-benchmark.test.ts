import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/tool-comparison-benchmark/report.json`;

describe("tool comparison benchmark", () => {
  it("shows why touch-browser is stronger than a markdown-only baseline on official public docs", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly sampleCount: number;
      readonly successfulSampleCount: number;
      readonly positiveClaimCount: number;
      readonly negativeClaimCount: number;
      readonly surfaces: {
        readonly markdownBaseline: {
          readonly averageTokens: number;
          readonly positiveClaimSupportRate: number;
          readonly plausibleNegativeFalsePositiveRate: number;
        };
        readonly touchBrowserReadView: {
          readonly averageTokens: number;
          readonly positiveClaimSupportRate: number;
        };
        readonly touchBrowserCompact: {
          readonly averageTokens: number;
          readonly positiveClaimSupportRate: number;
        };
        readonly touchBrowserExtract: {
          readonly positiveClaimSupportRate: number;
          readonly plausibleNegativeFalsePositiveRate: number;
          readonly structuredCitationCoverageRate: number;
          readonly stableRefCoverageRate: number;
          readonly averageSupportScore: number;
        };
      };
    }>(reportPath);

    expect(["competitive-validated", "partial"]).toContain(report.status);
    expect(report.sampleCount).toBeGreaterThanOrEqual(4);
    expect(report.successfulSampleCount).toBe(report.sampleCount);
    expect(report.positiveClaimCount).toBeGreaterThan(0);
    expect(report.negativeClaimCount).toBeGreaterThan(0);
    expect(
      report.surfaces.touchBrowserExtract.positiveClaimSupportRate,
    ).toBeGreaterThanOrEqual(0.75);
    expect(
      report.surfaces.touchBrowserExtract.plausibleNegativeFalsePositiveRate,
    ).toBeLessThanOrEqual(
      report.surfaces.markdownBaseline.plausibleNegativeFalsePositiveRate,
    );
    expect(
      report.surfaces.touchBrowserExtract.structuredCitationCoverageRate,
    ).toBe(1);
    expect(report.surfaces.touchBrowserExtract.stableRefCoverageRate).toBe(1);
    expect(
      report.surfaces.touchBrowserExtract.averageSupportScore,
    ).toBeGreaterThan(0.7);
    expect(report.surfaces.touchBrowserCompact.averageTokens).toBeLessThan(
      report.surfaces.markdownBaseline.averageTokens,
    );
    expect(
      report.surfaces.touchBrowserReadView.positiveClaimSupportRate,
    ).toBeGreaterThanOrEqual(
      report.surfaces.markdownBaseline.positiveClaimSupportRate,
    );
  }, 60_000);
});
