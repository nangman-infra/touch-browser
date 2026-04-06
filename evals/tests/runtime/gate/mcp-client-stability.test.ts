import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { createMcpClient } from "../../../../scripts/lib/mcp-client.mjs";

describe("mcp client stability", () => {
  const clients: Array<ReturnType<typeof createMcpClient>> = [];

  afterEach(async () => {
    await Promise.allSettled(clients.map((client) => client.close()));
    clients.length = 0;
  });

  it("rejects malformed bridge responses instead of hanging", async () => {
    const cwd = await mkdtemp(path.join(tmpdir(), "tb-mcp-malformed-"));

    const client = createMcpClient({
      cwd,
      bridgeCommand: "printf 'not-json\\n'; sleep 5",
      clientInfo: { name: "vitest", version: "0.0.0" },
      requestTimeoutMs: 5_000,
    });
    clients.push(client);

    await expect(client.initialize()).rejects.toThrow(
      "Invalid MCP bridge JSON response",
    );
  });

  it("times out stalled bridge calls instead of waiting forever", async () => {
    const cwd = await mkdtemp(path.join(tmpdir(), "tb-mcp-timeout-"));
    const scriptPath = path.join(cwd, "bridge.js");

    await writeFile(
      scriptPath,
      [
        'process.stdin.setEncoding("utf8");',
        "process.stdin.on('data', () => {",
        "  // Intentionally stall without replying.",
        "});",
      ].join("\n"),
      "utf8",
    );

    const client = createMcpClient({
      cwd,
      bridgeCommand: `node ${JSON.stringify(scriptPath)}`,
      clientInfo: { name: "vitest", version: "0.0.0" },
      requestTimeoutMs: 200,
    });
    clients.push(client);

    await expect(client.initialize()).rejects.toThrow(
      "MCP call timed out after 200ms: initialize",
    );
  });
});
