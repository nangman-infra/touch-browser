import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/observation-metrics/report.json`;

describe("observation metrics baseline", () => {
  it("keeps must-contain recall intact while reducing visible-text tokens", async () => {
    const report = await readJsonFile<{
      readonly fixtureCount: number;
      readonly averageHtmlReductionRatio: number;
      readonly averageCleanedDomReductionRatio: number;
      readonly averageHtmlTokenizerReductionRatio: number;
      readonly averageCleanedDomTokenizerReductionRatio: number;
      readonly averageVisibleTextTokenizerReductionRatio: number;
      readonly averageReadingHtmlTokenizerReductionRatio: number;
      readonly averageReadingCleanedDomTokenizerReductionRatio: number;
      readonly medianHtmlReductionRatio: number;
      readonly medianCleanedDomReductionRatio: number;
      readonly medianHtmlTokenizerReductionRatio: number;
      readonly medianCleanedDomTokenizerReductionRatio: number;
      readonly minHtmlReductionRatio: number;
      readonly minCleanedDomReductionRatio: number;
      readonly minHtmlTokenizerReductionRatio: number;
      readonly minCleanedDomTokenizerReductionRatio: number;
      readonly averageVisibleTextRatio: number;
      readonly averageMustContainRecall: number;
      readonly fixtures: ReadonlyArray<{
        readonly id: string;
        readonly htmlReductionRatio: number;
        readonly cleanedDomReductionRatio: number;
        readonly htmlTokenizerReductionRatio: number;
        readonly cleanedDomTokenizerReductionRatio: number;
        readonly readingHtmlTokenizerReductionRatio: number;
        readonly readingCleanedDomTokenizerReductionRatio: number;
        readonly reductionRatio: number;
        readonly mustContainRecall: number;
      }>;
    }>(reportPath);

    expect(report.fixtureCount).toBeGreaterThanOrEqual(30);
    expect(report.averageMustContainRecall).toBe(1);
    expect(report.minHtmlReductionRatio).toBeGreaterThan(1.8);
    expect(report.averageHtmlReductionRatio).toBeGreaterThan(2.2);
    expect(report.medianHtmlReductionRatio).toBeGreaterThan(2.2);
    expect(report.minCleanedDomReductionRatio).toBeGreaterThan(1);
    expect(report.averageCleanedDomReductionRatio).toBeGreaterThan(1.3);
    expect(report.medianCleanedDomReductionRatio).toBeGreaterThan(1.2);
    expect(report.minHtmlTokenizerReductionRatio).toBeGreaterThan(1.6);
    expect(report.averageHtmlTokenizerReductionRatio).toBeGreaterThan(2.4);
    expect(report.medianHtmlTokenizerReductionRatio).toBeGreaterThan(2.2);
    expect(report.minCleanedDomTokenizerReductionRatio).toBeGreaterThan(1.2);
    expect(report.averageCleanedDomTokenizerReductionRatio).toBeGreaterThan(8);
    expect(report.averageReadingHtmlTokenizerReductionRatio).toBeGreaterThan(
      report.averageHtmlTokenizerReductionRatio,
    );
    expect(
      report.averageReadingCleanedDomTokenizerReductionRatio,
    ).toBeGreaterThan(report.averageCleanedDomTokenizerReductionRatio);
    expect(report.averageVisibleTextRatio).toBeGreaterThan(0.9);
    expect(
      report.fixtures.every(
        (fixture) =>
          fixture.mustContainRecall === 1 &&
          fixture.cleanedDomReductionRatio > 1 &&
          fixture.htmlTokenizerReductionRatio > 1.6 &&
          fixture.cleanedDomTokenizerReductionRatio > 1.2 &&
          fixture.readingHtmlTokenizerReductionRatio >=
            fixture.htmlTokenizerReductionRatio &&
          fixture.readingCleanedDomTokenizerReductionRatio >=
            fixture.cleanedDomTokenizerReductionRatio,
      ),
    ).toBe(true);
  });
});
