import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/live-observation-metrics/report.json`;

describe("live observation metrics baseline", () => {
  it("keeps local live acquisition and browser paths above minimum compression and recall thresholds", async () => {
    const report = await readJsonFile<{
      readonly fixtureCount: number;
      readonly averageRuntimeHtmlTokenizerReductionRatio: number;
      readonly averageRuntimeCleanedDomTokenizerReductionRatio: number;
      readonly averageRuntimeReadingHtmlTokenizerReductionRatio: number;
      readonly averageRuntimeReadingCleanedDomTokenizerReductionRatio: number;
      readonly averageBrowserHtmlTokenizerReductionRatio: number;
      readonly averageBrowserCleanedDomTokenizerReductionRatio: number;
      readonly averageBrowserReadingHtmlTokenizerReductionRatio: number;
      readonly averageBrowserReadingCleanedDomTokenizerReductionRatio: number;
      readonly averageRuntimeMustContainRecall: number;
      readonly averageBrowserMustContainRecall: number;
    }>(reportPath);

    expect(report.fixtureCount).toBeGreaterThanOrEqual(3);
    expect(report.averageRuntimeMustContainRecall).toBe(1);
    expect(report.averageBrowserMustContainRecall).toBe(1);
    expect(report.averageRuntimeHtmlTokenizerReductionRatio).toBeGreaterThan(
      1.8,
    );
    expect(
      report.averageRuntimeCleanedDomTokenizerReductionRatio,
    ).toBeGreaterThan(1.2);
    expect(
      report.averageRuntimeReadingHtmlTokenizerReductionRatio,
    ).toBeGreaterThanOrEqual(report.averageRuntimeHtmlTokenizerReductionRatio);
    expect(
      report.averageRuntimeReadingCleanedDomTokenizerReductionRatio,
    ).toBeGreaterThanOrEqual(
      report.averageRuntimeCleanedDomTokenizerReductionRatio,
    );
    expect(report.averageBrowserHtmlTokenizerReductionRatio).toBeGreaterThan(
      1.8,
    );
    expect(
      report.averageBrowserCleanedDomTokenizerReductionRatio,
    ).toBeGreaterThan(1.2);
    expect(
      report.averageBrowserReadingHtmlTokenizerReductionRatio,
    ).toBeGreaterThanOrEqual(report.averageBrowserHtmlTokenizerReductionRatio);
    expect(
      report.averageBrowserReadingCleanedDomTokenizerReductionRatio,
    ).toBeGreaterThanOrEqual(
      report.averageBrowserCleanedDomTokenizerReductionRatio,
    );
  });
});
