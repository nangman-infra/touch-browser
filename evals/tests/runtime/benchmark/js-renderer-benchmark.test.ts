import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/js-renderer-benchmark/report.json`;

describe("js renderer benchmark", () => {
  it("keeps live JS docs and app pages on the expected extraction and auto-route path", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly sampleCount: number;
      readonly successfulSampleCount: number;
      readonly docsSampleCount: number;
      readonly docsSupportedCount: number;
      readonly docsSupportedRate: number;
      readonly mustContainRecallRate: number;
      readonly appAutoPlaywrightCount: number;
      readonly appAutoPlaywrightRate: number;
      readonly samples: ReadonlyArray<{
        readonly id: string;
        readonly pageType: string;
        readonly error?: string;
        readonly open?: {
          readonly sourceType: string;
        };
        readonly mainOnly?: {
          readonly passed: boolean;
          readonly tokens: number;
        };
        readonly extract?: {
          readonly verdict: string | null;
          readonly passed: boolean;
        };
      }>;
    }>(reportPath);

    expect(report.sampleCount).toBeGreaterThanOrEqual(4);
    expect(report.successfulSampleCount).toBe(report.sampleCount);
    expect(report.docsSampleCount).toBeGreaterThanOrEqual(3);
    expect(report.docsSupportedCount).toBe(report.docsSampleCount);
    expect(report.docsSupportedRate).toBe(1);
    expect(report.mustContainRecallRate).toBeGreaterThanOrEqual(0.9);
    expect(report.appAutoPlaywrightCount).toBeGreaterThanOrEqual(1);
    expect(report.appAutoPlaywrightRate).toBe(1);
    expect(report.status).toBe("js-renderer-validated");
    expect(report.samples.every((sample) => !sample.error)).toBe(true);
    expect(
      report.samples
        .filter((sample) => sample.pageType === "app")
        .every(
          (sample) =>
            sample.open?.sourceType === "playwright" &&
            sample.mainOnly?.passed === true &&
            Number(sample.mainOnly?.tokens ?? 0) <= 32,
        ),
    ).toBe(true);
    expect(
      report.samples
        .filter((sample) => sample.pageType === "docs")
        .every(
          (sample) =>
            sample.mainOnly?.passed === true &&
            sample.extract?.verdict === "evidence-supported" &&
            sample.extract?.passed === true,
        ),
    ).toBe(true);
  });
});
