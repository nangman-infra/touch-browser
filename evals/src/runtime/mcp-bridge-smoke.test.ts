import { type ChildProcessWithoutNullStreams, spawn } from "node:child_process";

import { afterEach, describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";

describe("mcp bridge smoke", () => {
  const clients: ChildProcessWithoutNullStreams[] = [];

  afterEach(async () => {
    await Promise.allSettled(
      clients.map(
        (child) =>
          new Promise<void>((resolve) => {
            if (child.killed || child.exitCode !== null) {
              resolve();
              return;
            }
            child.stdin.end();
            child.once("close", () => resolve());
            setTimeout(() => {
              if (child.exitCode === null) {
                child.kill("SIGTERM");
              }
            }, 250);
          }),
      ),
    );
    clients.length = 0;
  });

  it("exposes touch-browser tools over MCP stdio", async () => {
    const child = spawn(
      "zsh",
      ["-lic", "node scripts/touch-browser-mcp-bridge.mjs"],
      {
        cwd: repoRoot,
        stdio: ["pipe", "pipe", "pipe"],
      },
    );
    clients.push(child);

    const call = createRpcCaller(child);

    const initialize = await call<{ protocolVersion: string }>("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: {
        name: "vitest",
        version: "0.0.0",
      },
    });
    expect(initialize.protocolVersion).toBe("2025-06-18");

    const tools = await call<{ tools: Array<{ readonly name: string }> }>(
      "tools/list",
      {},
    );
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_open",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_submit",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_tab_list",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_tab_select",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_tab_close",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_checkpoint",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_profile",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_refresh",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_approve",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_type_secret",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) =>
          tool.name === "tb_telemetry_summary",
      ),
    ).toBe(true);

    const status = await call<{
      structuredContent: {
        status: string;
        daemon: boolean;
      };
    }>("tools/call", {
      name: "tb_status",
      arguments: {},
    });
    expect(status.structuredContent.status).toBe("ready");
    expect(status.structuredContent.daemon).toBe(true);
  }, 20_000);
});

function createRpcCaller(child: ChildProcessWithoutNullStreams) {
  const pending = new Map<
    number,
    {
      resolve: (value: unknown) => void;
      reject: (error: Error) => void;
    }
  >();
  let nextId = 0;
  let stdoutBuffer = "";
  let stderrBuffer = "";

  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdoutBuffer += chunk;
    const lines = stdoutBuffer.split("\n");
    stdoutBuffer = lines.pop() ?? "";

    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }
      const payload = JSON.parse(line);
      const handler = pending.get(payload.id);
      if (!handler) {
        continue;
      }
      pending.delete(payload.id);
      if (payload.error) {
        handler.reject(new Error(payload.error.message));
      } else {
        handler.resolve(payload.result);
      }
    }
  });

  child.stderr.setEncoding("utf8");
  child.stderr.on("data", (chunk) => {
    stderrBuffer += chunk;
  });

  child.on("exit", (code) => {
    if (pending.size === 0) {
      return;
    }
    const error = new Error(
      `mcp bridge exited with code ${code ?? -1}: ${stderrBuffer.trim()}`,
    );
    for (const handler of pending.values()) {
      handler.reject(error);
    }
    pending.clear();
  });

  return async function call<T>(
    method: string,
    params: Record<string, unknown>,
  ): Promise<T> {
    const id = ++nextId;
    const payload = JSON.stringify({
      jsonrpc: "2.0",
      id,
      method,
      params,
    });

    return await new Promise<T>((resolve, reject) => {
      pending.set(id, {
        resolve: (value) => resolve(value as T),
        reject,
      });
      child.stdin.write(`${payload}\n`, "utf8", (error) => {
        if (error) {
          pending.delete(id);
          reject(error);
        }
      });
    });
  };
}
