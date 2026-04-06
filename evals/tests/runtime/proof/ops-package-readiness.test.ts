import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/ops-package-readiness/report.json`;

describe("ops package readiness", () => {
  it("keeps self-hosted pilot operations and security artifacts complete", async () => {
    const report = await readJsonFile<{
      readonly status: string;
      readonly checks: {
        readonly docsReady: boolean;
        readonly scriptsReady: boolean;
        readonly deployArtifactsReady: boolean;
        readonly containerRuntimeReady: boolean;
        readonly secretLifecycleReady: boolean;
        readonly retentionRunbookReady: boolean;
        readonly upgradeRunbookReady: boolean;
        readonly hardeningReady: boolean;
        readonly healthcheckReady: boolean;
      };
    }>(reportPath);

    expect(report.status).toBe("ops-package-ready");
    expect(report.checks.docsReady).toBe(true);
    expect(report.checks.scriptsReady).toBe(true);
    expect(report.checks.deployArtifactsReady).toBe(true);
    expect(report.checks.containerRuntimeReady).toBe(true);
    expect(report.checks.secretLifecycleReady).toBe(true);
    expect(report.checks.retentionRunbookReady).toBe(true);
    expect(report.checks.upgradeRunbookReady).toBe(true);
    expect(report.checks.hardeningReady).toBe(true);
    expect(report.checks.healthcheckReady).toBe(true);
  });
});
