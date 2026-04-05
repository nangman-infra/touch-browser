import { readFileSync } from "node:fs";

import { describe, expect, it } from "vitest";

import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/adversarial-benchmark/report.json`;

describe("adversarial benchmark", () => {
  it("keeps the verified extractor on the expected side of adversarial official-doc claims", () => {
    const report = JSON.parse(readFileSync(reportPath, "utf8")) as {
      status: string;
      sampleCount: number;
      successfulSampleCount: number;
      verifiedExactVerdictAccuracy: number;
      scenarios: Array<{
        expectedVerdict: string;
        verifiedVerdict?: string;
      }>;
    };

    expect(report.status).toBe("adversarial-validated");
    expect(report.successfulSampleCount).toBe(report.sampleCount);
    expect(report.verifiedExactVerdictAccuracy).toBe(1);
    expect(
      report.scenarios.every(
        (scenario) => scenario.verifiedVerdict === scenario.expectedVerdict,
      ),
    ).toBe(true);
  });
});
