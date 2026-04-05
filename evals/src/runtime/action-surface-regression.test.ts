import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/action-surface-regression/report.json`;

describe("action surface regression proxy", () => {
  it("keeps the core action surface materially smaller and less ambiguous than an expanded surface", async () => {
    const report = await readJsonFile<{
      readonly coreSurfaceCount: number;
      readonly expandedSurfaceCount: number;
      readonly averageCoreWrongToolOpportunityRate: number;
      readonly averageExpandedWrongToolOpportunityRate: number;
      readonly wrongToolOpportunityReductionRate: number;
    }>(reportPath);

    expect(report.coreSurfaceCount).toBeLessThan(report.expandedSurfaceCount);
    expect(report.averageCoreWrongToolOpportunityRate).toBeLessThan(
      report.averageExpandedWrongToolOpportunityRate,
    );
    expect(report.wrongToolOpportunityReductionRate).toBeGreaterThanOrEqual(
      0.3,
    );
  });
});
