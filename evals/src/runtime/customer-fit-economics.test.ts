import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/customer-fit-economics/report.json`;

describe("customer fit and economics validation", () => {
  it("keeps the research wedge at validated-alpha or better against internal gate scores", async () => {
    const report = await readJsonFile<{
      readonly wedge: string;
      readonly qualityScore: number;
      readonly controlScore: number;
      readonly economicsScore: number;
      readonly status: string;
    }>(reportPath);

    expect(report.wedge).toBe("Research Agent Platform Teams");
    expect(report.qualityScore).toBeGreaterThanOrEqual(0.9);
    expect(report.controlScore).toBeGreaterThanOrEqual(0.6);
    expect(report.economicsScore).toBeGreaterThanOrEqual(0.6);
    expect(["validated-alpha", "conditional"]).toContain(report.status);
  });
});
