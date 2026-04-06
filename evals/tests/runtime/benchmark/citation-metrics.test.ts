import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/citation-metrics/report.json`;

describe("citation metrics baseline", () => {
  it("keeps claim classification and citation payload precision intact", async () => {
    const report = await readJsonFile<{
      readonly fixtureCount: number;
      readonly averageClassificationPrecision: number;
      readonly averageClassificationRecall: number;
      readonly averageCitationPrecision: number;
      readonly averageCitationRecall: number;
      readonly averageUnsupportedPrecision: number;
      readonly averageUnsupportedRecall: number;
      readonly averageSupportReferencePrecision: number;
      readonly sourceAlignmentRate: number;
      readonly fixtures: ReadonlyArray<{
        readonly id: string;
        readonly classificationPrecision: number;
        readonly classificationRecall: number;
        readonly citationPrecision: number;
        readonly citationRecall: number;
        readonly supportReferencePrecision: number;
        readonly sourceAligned: boolean;
      }>;
    }>(reportPath);

    expect(report.fixtureCount).toBeGreaterThanOrEqual(30);
    expect(report.averageClassificationPrecision).toBeGreaterThanOrEqual(0.95);
    expect(report.averageClassificationRecall).toBeGreaterThanOrEqual(0.95);
    expect(report.averageCitationPrecision).toBeGreaterThanOrEqual(0.95);
    expect(report.averageCitationRecall).toBeGreaterThanOrEqual(0.95);
    expect(report.averageUnsupportedPrecision).toBeGreaterThanOrEqual(0.95);
    expect(report.averageUnsupportedRecall).toBeGreaterThanOrEqual(0.95);
    expect(report.averageSupportReferencePrecision).toBe(1);
    expect(report.sourceAlignmentRate).toBe(1);
    expect(
      report.fixtures.every(
        (fixture) =>
          fixture.classificationPrecision >= 0.95 &&
          fixture.classificationRecall >= 0.95 &&
          fixture.citationPrecision >= 0.95 &&
          fixture.citationRecall >= 0.95 &&
          fixture.supportReferencePrecision === 1 &&
          fixture.sourceAligned,
      ),
    ).toBe(true);
  });
});
