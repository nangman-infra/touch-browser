import { spawn } from "node:child_process";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";

describe("serve mode smoke", () => {
  it("serves stdio JSON-RPC for pilot integrations", async () => {
    const child = spawn(
      "zsh",
      ["-lic", "cargo run -q -p touch-browser-cli -- serve"],
      {
        cwd: repoRoot,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );

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
});
