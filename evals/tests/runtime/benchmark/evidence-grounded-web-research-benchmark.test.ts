import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/evidence-grounded-web-research-benchmark/report.json`;

describe("evidence-grounded web research benchmark", () => {
  it("keeps the seed category benchmark production-credible", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly seedClaimOrFixtureCount: number;
      readonly targetClaimCount: number;
      readonly qualityGates: {
        readonly publicWebSupportedClaimRate: number;
        readonly realUserSupportedClaimRate: number;
        readonly verifiedAdversarialAccuracy: number;
        readonly unsafeAutoAnswerCount: number;
        readonly citationPrecision: number;
        readonly unsupportedPrecision: number;
        readonly supportReferencePrecision: number;
        readonly touchBrowserFalsePositiveRate: number;
        readonly markdownFalsePositiveRate: number;
      };
    }>(reportPath);

    expect(report.status).toBe("seed-validated");
    expect(report.seedClaimOrFixtureCount).toBeGreaterThanOrEqual(50);
    expect(report.targetClaimCount).toBe(1000);
    expect(
      report.qualityGates.publicWebSupportedClaimRate,
    ).toBeGreaterThanOrEqual(0.95);
    expect(
      report.qualityGates.realUserSupportedClaimRate,
    ).toBeGreaterThanOrEqual(0.95);
    expect(
      report.qualityGates.verifiedAdversarialAccuracy,
    ).toBeGreaterThanOrEqual(0.95);
    expect(report.qualityGates.unsafeAutoAnswerCount).toBe(0);
    expect(report.qualityGates.citationPrecision).toBeGreaterThanOrEqual(0.95);
    expect(report.qualityGates.unsupportedPrecision).toBeGreaterThanOrEqual(
      0.95,
    );
    expect(
      report.qualityGates.supportReferencePrecision,
    ).toBeGreaterThanOrEqual(0.95);
    expect(
      report.qualityGates.touchBrowserFalsePositiveRate,
    ).toBeLessThanOrEqual(report.qualityGates.markdownFalsePositiveRate);
  });
});
