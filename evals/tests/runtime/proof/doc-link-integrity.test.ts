import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/doc-link-integrity/report.json`;

describe("doc link integrity", () => {
  it("keeps tracked markdown links live and internally resolvable", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly markdownFileCount: number;
      readonly relativeFailureCount: number;
      readonly anchorFailureCount: number;
      readonly externalLinkCount: number;
      readonly externalCheckedCount: number;
      readonly externalSuccessRate: number;
      readonly relativeFailures: readonly unknown[];
      readonly anchorFailures: readonly unknown[];
      readonly externalFailures: readonly unknown[];
    }>(reportPath);

    expect(report.status).toBe("ok");
    expect(report.markdownFileCount).toBeGreaterThan(5);
    expect(report.relativeFailureCount).toBe(0);
    expect(report.anchorFailureCount).toBe(0);
    expect(report.externalLinkCount).toBeGreaterThan(0);
    expect(report.externalCheckedCount).toBeGreaterThan(0);
    expect(report.externalSuccessRate).toBe(1);
    expect(report.relativeFailures).toHaveLength(0);
    expect(report.anchorFailures).toHaveLength(0);
    expect(report.externalFailures).toHaveLength(0);
  }, 60_000);
});
