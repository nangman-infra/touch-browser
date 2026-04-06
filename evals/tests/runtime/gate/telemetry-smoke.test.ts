import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";
import { spawnShellCommand } from "../support/shell.js";

describe("pilot telemetry smoke", () => {
  it("records CLI operations and exposes summary/recent views", async () => {
    const tempDir = mkdtempSync(
      path.join(tmpdir(), "touch-browser-telemetry-"),
    );
    const telemetryDb = path.join(tempDir, "pilot.sqlite");
    const binary = path.join(repoRoot, "target", "debug", "touch-browser");
    const env = {
      ...process.env,
      TOUCH_BROWSER_TELEMETRY_DB: telemetryDb,
      TOUCH_BROWSER_TELEMETRY_SURFACE: "cli-smoke",
    };

    await runShell("cargo build -q -p touch-browser-cli", env);
    await runShell(
      `${binary} open fixture://research/static-docs/getting-started`,
      env,
    );
    await runShell(
      `${binary} compact-view fixture://research/static-docs/getting-started`,
      env,
    );

    const summary = JSON.parse(
      await runShell(`${binary} telemetry-summary`, env),
    );
    expect(summary.summary.totalEvents).toBeGreaterThanOrEqual(2);
    expect(summary.summary.surfaceCounts["cli-smoke"]).toBeGreaterThanOrEqual(
      2,
    );

    const recent = JSON.parse(
      await runShell(`${binary} telemetry-recent --limit 4`, env),
    );
    expect(recent.events.length).toBeGreaterThanOrEqual(2);
    expect(
      recent.events.some(
        (event: { readonly operation: string }) => event.operation === "open",
      ),
    ).toBe(true);
  }, 60_000);
});

async function runShell(command: string, env: NodeJS.ProcessEnv) {
  const child = spawnShellCommand(command, {
    cwd: repoRoot,
    env,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const stdout: Buffer[] = [];
  const stderr: Buffer[] = [];
  child.stdout.on("data", (chunk) => stdout.push(chunk));
  child.stderr.on("data", (chunk) => stderr.push(chunk));

  const code = await new Promise<number>((resolve, reject) => {
    child.on("error", reject);
    child.on("close", (exitCode) => resolve(exitCode ?? 1));
  });

  if (code !== 0) {
    throw new Error(Buffer.concat(stderr).toString("utf8"));
  }

  return Buffer.concat(stdout).toString("utf8").trim();
}
