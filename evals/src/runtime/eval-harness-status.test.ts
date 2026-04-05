import { describe, expect, it } from "vitest";

import { evalHarnessStatus } from "../index.js";

describe("eval harness status", () => {
  it("summarizes readiness, proxy, and workflow artifacts instead of returning a scaffold marker", () => {
    const status = evalHarnessStatus();

    expect(status.package).toBe("evals");
    expect(status.status).toBe("active");
    expect(status.readiness.status).toBeTruthy();
    expect(status.proxy.coreTaskCount).toBeGreaterThanOrEqual(4);
    expect(status.workflows.reference).toBe("ok");
    expect(status.workflows.staged).toBe("ok");
    expect(status.workflows.publicReference).toBe("ok");
    expect(status.workflows.realUserResearch).toBe("real-user-validated");
    expect(status.coverage.runtimeReady).toBe(true);
    expect(status.coverage.mixedSourceReady).toBe(true);
    expect(status.coverage.publicProofReady).toBe(true);
    expect(status.coverage.realUserResearchReady).toBe(true);
  });
});
