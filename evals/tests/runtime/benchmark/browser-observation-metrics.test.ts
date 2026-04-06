import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/browser-observation-metrics/report.json`;

describe("browser observation metrics baseline", () => {
  it("keeps browser-backed compact snapshots token-efficient", async () => {
    const report = await readJsonFile<{
      readonly fixtureCount: number;
      readonly averageHtmlTokenizerReductionRatio: number;
      readonly averageCleanedDomTokenizerReductionRatio: number;
      readonly averageVisibleTextTokenizerReductionRatio: number;
      readonly averageReadingHtmlTokenizerReductionRatio: number;
      readonly averageReadingCleanedDomTokenizerReductionRatio: number;
      readonly averageMustContainRecall: number;
      readonly fixtures: ReadonlyArray<{
        readonly id: string;
        readonly sourceType: string;
        readonly htmlTokenizerReductionRatio: number;
        readonly cleanedDomTokenizerReductionRatio: number;
        readonly readingHtmlTokenizerReductionRatio: number;
        readonly readingCleanedDomTokenizerReductionRatio: number;
        readonly visibleTextTokenizerReductionRatio: number;
        readonly mustContainRecall: number;
        readonly truncated: boolean;
      }>;
    }>(reportPath);

    expect(report.fixtureCount).toBeGreaterThanOrEqual(8);
    expect(report.averageMustContainRecall).toBe(1);
    expect(report.averageHtmlTokenizerReductionRatio).toBeGreaterThan(2);
    expect(report.averageCleanedDomTokenizerReductionRatio).toBeGreaterThan(
      1.3,
    );
    expect(report.averageReadingHtmlTokenizerReductionRatio).toBeGreaterThan(
      report.averageHtmlTokenizerReductionRatio,
    );
    expect(
      report.averageReadingCleanedDomTokenizerReductionRatio,
    ).toBeGreaterThan(report.averageCleanedDomTokenizerReductionRatio);
    expect(report.averageVisibleTextTokenizerReductionRatio).toBeGreaterThan(
      0.45,
    );
    expect(
      report.fixtures.every(
        (fixture) =>
          fixture.sourceType === "playwright" &&
          fixture.htmlTokenizerReductionRatio > 1.5 &&
          fixture.cleanedDomTokenizerReductionRatio > 1.1 &&
          fixture.readingHtmlTokenizerReductionRatio >=
            fixture.htmlTokenizerReductionRatio &&
          fixture.readingCleanedDomTokenizerReductionRatio >=
            fixture.cleanedDomTokenizerReductionRatio &&
          fixture.mustContainRecall === 1 &&
          fixture.truncated === false,
      ),
    ).toBe(true);
  });
});
