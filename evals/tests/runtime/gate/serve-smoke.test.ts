import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";
import { spawnShellCommand } from "../support/shell.js";

describe("serve mode smoke", () => {
  it("serves stdio JSON-RPC for pilot integrations", async () => {
    const child = spawnShellCommand("target/debug/touch-browser serve", {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    });

    const lines: string[] = [];
    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      for (const line of chunk.split("\n")) {
        if (line.trim().length > 0) {
          lines.push(line.trim());
        }
      }
    });

    child.stdin.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method: "runtime.status",
        params: {},
      })}\n`,
    );
    child.stdin.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: 2,
        method: "runtime.open",
        params: {
          target: "fixture://research/static-docs/getting-started",
        },
      })}\n`,
    );
    child.stdin.end();

    await new Promise<void>((resolve, reject) => {
      child.on("error", reject);
      child.on("close", (code) => {
        if (code !== 0) {
          reject(new Error(`serve exited with code ${code}`));
          return;
        }

        resolve();
      });
    });

    expect(lines.length).toBeGreaterThanOrEqual(2);
    const [statusLine, openLine] = lines;
    expect(statusLine).toBeDefined();
    expect(openLine).toBeDefined();
    if (statusLine === undefined || openLine === undefined) {
      throw new Error("serve smoke expected status and open responses");
    }

    const status = JSON.parse(statusLine);
    const open = JSON.parse(openLine);

    expect(status.result.status).toBe("ready");
    expect(status.result.methods).toContain("runtime.open");
    expect(open.result.action).toBe("open");
    expect(open.result.status).toBe("succeeded");
    expect(open.result.output.source.sourceUrl).toBe(
      "fixture://research/static-docs/getting-started",
    );
  }, 20_000);

  it("keeps JSON-RPC error payloads on stdout and reserves stderr for process failures", async () => {
    const child = spawnShellCommand("target/debug/touch-browser serve", {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    });

    let stdoutBuffer = "";
    let stderrBuffer = "";

    child.stdout.setEncoding("utf8");
    child.stdout.on("data", (chunk) => {
      stdoutBuffer += chunk;
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderrBuffer += chunk;
    });

    child.stdin.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: 99,
        method: "runtime.unknown",
        params: {},
      })}\n`,
    );
    child.stdin.end();

    await new Promise<void>((resolve, reject) => {
      child.on("error", reject);
      child.on("close", (code) => {
        if (code !== 0) {
          reject(new Error(`serve exited with code ${code}`));
          return;
        }

        resolve();
      });
    });

    const responseLine = stdoutBuffer
      .split("\n")
      .map((line) => line.trim())
      .find((line) => line.length > 0);
    expect(responseLine).toBeTruthy();
    if (!responseLine) {
      throw new Error("serve smoke expected one JSON-RPC error response");
    }

    const response = JSON.parse(responseLine);
    expect(response.error.message).toContain("Unsupported serve method");
    expect(stderrBuffer.trim()).toBe("");
  }, 20_000);
});
