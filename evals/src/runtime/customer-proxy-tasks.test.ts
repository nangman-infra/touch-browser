import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/customer-proxy-tasks/report.json`;

describe("customer proxy task suite", () => {
  it("keeps the research-agent wedge backed by repeatable proxy tasks", async () => {
    const report = await readJsonFile<{
      readonly coreTaskCount: number;
      readonly optionalTaskCount: number;
      readonly coreProxySuccessRate: number;
      readonly extendedProxySuccessRate: number;
      readonly status: string;
      readonly tasks: ReadonlyArray<{
        readonly id: string;
        readonly status: string;
      }>;
    }>(reportPath);

    expect(report.coreTaskCount).toBeGreaterThanOrEqual(4);
    expect(report.coreProxySuccessRate).toBe(1);
    expect(report.extendedProxySuccessRate).toBeGreaterThanOrEqual(0.6);
    expect(["proxy-validated", "core-proxy-validated"]).toContain(
      report.status,
    );
    expect(report.tasks.every((task) => task.status === "passed")).toBe(true);
    expect(report.optionalTaskCount).toBeGreaterThanOrEqual(0);
  });
});
