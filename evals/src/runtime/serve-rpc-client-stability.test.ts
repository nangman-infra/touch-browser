import { mkdtemp, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { createServeRpcClient } from "../../../scripts/lib/serve-rpc-client.mjs";

describe("serve rpc client stability", () => {
  const clients: Array<ReturnType<typeof createServeRpcClient>> = [];

  afterEach(async () => {
    await Promise.allSettled(clients.map((client) => client.close()));
    clients.length = 0;
  });

  it("rejects malformed serve responses instead of hanging", async () => {
    const cwd = await mkdtemp(path.join(tmpdir(), "tb-serve-malformed-"));

    const client = createServeRpcClient({
      cwd,
      serveCommand: "printf 'not-json\\n'; sleep 5",
      requestTimeoutMs: 5_000,
    });
    clients.push(client);

    await expect(client.call("runtime.status", {})).rejects.toThrow(
      "Invalid serve daemon JSON response",
    );
  });

  it("times out stalled serve calls instead of waiting forever", async () => {
    const cwd = await mkdtemp(path.join(tmpdir(), "tb-serve-timeout-"));
    const scriptPath = path.join(cwd, "serve.js");

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

    const client = createServeRpcClient({
      cwd,
      serveCommand: `node ${JSON.stringify(scriptPath)}`,
      requestTimeoutMs: 200,
    });
    clients.push(client);

    await expect(client.call("runtime.status", {})).rejects.toThrow(
      "serve RPC call timed out after 200ms: runtime.status",
    );
  });
});
