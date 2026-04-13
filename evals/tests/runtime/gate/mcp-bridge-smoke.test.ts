import type { ChildProcessWithoutNullStreams } from "node:child_process";
import fs from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";

import { afterEach, describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";
import { spawnShellCommand } from "../support/shell.js";

type BridgeServeClient = {
  readonly child: ChildProcessWithoutNullStreams;
  ensureReady<T = unknown>(): Promise<T>;
  call<T = unknown>(
    method: string,
    params?: Record<string, unknown>,
  ): Promise<T>;
  close(): Promise<void>;
};

const require = createRequire(import.meta.url);
const {
  createBridgeServeClient,
  resolveServeCommand,
}: {
  createBridgeServeClient: (options?: {
    cwd?: string;
    serveCommand?: string;
    requestTimeoutMs?: number;
  }) => BridgeServeClient;
  resolveServeCommand: (options?: {
    cwd?: string;
    env?: NodeJS.ProcessEnv;
  }) => string;
} = require("../../../../integrations/mcp/bridge/serve-client.mjs");

describe("mcp bridge smoke", () => {
  const clients: ChildProcessWithoutNullStreams[] = [];
  const serveClients: BridgeServeClient[] = [];

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
    await Promise.allSettled(serveClients.map((client) => client.close()));
    serveClients.length = 0;
  });

  it("exposes touch-browser tools over MCP stdio", async () => {
    const child = spawnShellCommand("node integrations/mcp/bridge/index.mjs", {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    }) as ChildProcessWithoutNullStreams;
    clients.push(child);

    const call = createRpcCaller(child);

    const initialize = await call<{
      protocolVersion: string;
      instructions: string;
    }>("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: {
        name: "vitest",
        version: "0.0.0",
      },
    });
    expect(initialize.protocolVersion).toBe("2025-06-18");
    expect(initialize.instructions).toContain(
      "public docs and research web workflows",
    );
    expect(initialize.instructions).toContain(
      "MCP does not expose engine or headed controls",
    );

    const tools = await call<{
      tools: Array<{
        readonly name: string;
        readonly inputSchema?: {
          readonly properties?: Record<string, unknown>;
        };
      }>;
    }>("tools/list", {});
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_open",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_search",
      ),
    ).toBe(true);
    const searchTool = tools.tools.find(
      (tool: { readonly name: string }) => tool.name === "tb_search",
    );
    const openTool = tools.tools.find(
      (tool: { readonly name: string }) => tool.name === "tb_open",
    );
    const sessionCreateTool = tools.tools.find(
      (tool: { readonly name: string }) => tool.name === "tb_session_create",
    );
    const readViewTool = tools.tools.find(
      (tool: { readonly name: string }) => tool.name === "tb_read_view",
    );
    const extractTool = tools.tools.find(
      (tool: { readonly name: string }) => tool.name === "tb_extract",
    );
    const refreshTool = tools.tools.find(
      (tool: { readonly name: string }) => tool.name === "tb_refresh",
    );
    expect(searchTool).toBeDefined();
    expect(openTool).toBeDefined();
    expect(sessionCreateTool).toBeDefined();
    expect(readViewTool).toBeDefined();
    expect(extractTool).toBeDefined();
    expect(refreshTool).toBeDefined();
    expect(searchTool?.inputSchema?.properties).not.toHaveProperty("engine");
    expect(searchTool?.inputSchema?.properties).not.toHaveProperty("headed");
    expect(openTool?.inputSchema?.properties).not.toHaveProperty("headed");
    expect(sessionCreateTool?.inputSchema?.properties).not.toHaveProperty(
      "headed",
    );
    expect(readViewTool?.inputSchema?.properties).not.toHaveProperty("headed");
    expect(extractTool?.inputSchema?.properties).not.toHaveProperty("headed");
    expect(refreshTool?.inputSchema?.properties).not.toHaveProperty("headed");
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) =>
          tool.name === "tb_search_open_result",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_search_open_top",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_submit",
      ),
    ).toBe(true);
    expect(
      tools.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_read_view",
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
  }, 40_000);

  it("rejects managed engine or headed MCP arguments instead of forwarding them", async () => {
    const child = spawnShellCommand("node integrations/mcp/bridge/index.mjs", {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    }) as ChildProcessWithoutNullStreams;
    clients.push(child);

    const call = createRpcCaller(child);

    await call("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: {
        name: "vitest",
        version: "0.0.0",
      },
    });

    const headedError = await callError(child, "tools/call", {
      name: "tb_open",
      arguments: {
        target: "fixture://research/static-docs/getting-started",
        headed: true,
      },
    });
    expect(headedError.message).toContain("does not accept `headed` over MCP");

    const created = await call<{
      structuredContent: {
        sessionId: string;
      };
    }>("tools/call", {
      name: "tb_session_create",
      arguments: {},
    });

    const engineError = await callError(child, "tools/call", {
      name: "tb_search",
      arguments: {
        sessionId: created.structuredContent.sessionId,
        query: "MDN AbortController official",
        engine: "google",
      },
    });
    expect(engineError.message).toContain("does not accept `engine` over MCP");
  }, 40_000);

  it("exposes the same MCP bridge through the touch-browser mcp command", async () => {
    const child = spawnShellCommand(
      "cargo run -q -p touch-browser-cli -- mcp",
      {
        cwd: repoRoot,
        stdio: ["pipe", "pipe", "pipe"],
      },
    ) as ChildProcessWithoutNullStreams;
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
        (tool: { readonly name: string }) => tool.name === "tb_status",
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
  }, 40_000);

  it("returns MCP protocol errors on stdout without polluting stderr", async () => {
    const child = spawnShellCommand("node integrations/mcp/bridge/index.mjs", {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    }) as ChildProcessWithoutNullStreams;
    clients.push(child);

    const call = createRpcCaller(child);

    await call("initialize", {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: {
        name: "vitest",
        version: "0.0.0",
      },
    });

    const error = await callError(child, "tools/call", {
      name: "tb_missing",
      arguments: {},
    });
    expect(error.message).toContain("Unknown tool: tb_missing");
    expect(error.stderr.trim()).toBe("");
  }, 20_000);

  it("restarts the serve daemon after a crash on the next call", async () => {
    const serve = createBridgeServeClient({
      cwd: repoRoot,
      serveCommand: "target/debug/touch-browser serve",
    });
    serveClients.push(serve);

    const firstStatus = await serve.ensureReady<{ status: string }>();
    expect(firstStatus.status).toBe("ready");
    const firstPid = serve.child.pid;
    serve.child.kill("SIGTERM");

    await new Promise((resolve) => setTimeout(resolve, 300));

    const secondStatus = await serve.call<{ status: string }>(
      "runtime.status",
      {},
    );
    expect(secondStatus.status).toBe("ready");
    expect(serve.child.pid).not.toBe(firstPid);
  }, 20_000);

  it("prefers repo-local debug binaries before packaged standalone bundles in a repo checkout", () => {
    const tempRoot = fs.mkdtempSync(
      path.join(os.tmpdir(), "touch-browser-mcp-"),
    );
    fs.writeFileSync(path.join(tempRoot, "Cargo.toml"), "[workspace]\n");
    fs.writeFileSync(path.join(tempRoot, "package.json"), "{}\n");
    fs.mkdirSync(path.join(tempRoot, "core"), { recursive: true });
    fs.mkdirSync(path.join(tempRoot, "integrations"), { recursive: true });

    const debugBinaryPath = path.join(
      tempRoot,
      "target",
      "debug",
      "touch-browser",
    );
    fs.mkdirSync(path.dirname(debugBinaryPath), { recursive: true });
    fs.writeFileSync(debugBinaryPath, "#!/bin/sh\nexit 0\n");
    fs.chmodSync(debugBinaryPath, 0o755);

    const packagedBinaryPath = path.join(
      tempRoot,
      "dist",
      "standalone",
      "touch-browser-local-test",
      "bin",
      "touch-browser",
    );
    fs.mkdirSync(path.dirname(packagedBinaryPath), { recursive: true });
    fs.writeFileSync(packagedBinaryPath, "#!/bin/sh\nexit 0\n");
    fs.chmodSync(packagedBinaryPath, 0o755);

    const command = resolveServeCommand({
      cwd: tempRoot,
      env: {
        ...process.env,
        TOUCH_BROWSER_SERVE_COMMAND: "",
        TOUCH_BROWSER_SERVE_BINARY: "",
        PATH: "",
      },
    });

    expect(command).toContain("target/debug/touch-browser");
    expect(command.endsWith(" serve")).toBe(true);
  });

  it("fails fast when no touch-browser binary can be resolved", () => {
    const tempRoot = fs.mkdtempSync(
      path.join(os.tmpdir(), "touch-browser-missing-"),
    );

    expect(() =>
      resolveServeCommand({
        cwd: tempRoot,
        env: {
          ...process.env,
          TOUCH_BROWSER_SERVE_COMMAND: "",
          TOUCH_BROWSER_SERVE_BINARY: "",
          TOUCH_BROWSER_BINARY: "",
          PATH: "",
        },
      }),
    ).toThrow("Could not resolve a touch-browser binary");
  });
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

function callError(
  child: ChildProcessWithoutNullStreams,
  method: string,
  params: Record<string, unknown>,
) {
  let nextId = 10_000;
  let stdoutBuffer = "";
  let stderrBuffer = "";

  child.stdout.setEncoding("utf8");
  child.stderr.setEncoding("utf8");

  return new Promise<{ message: string; stderr: string }>((resolve, reject) => {
    const id = ++nextId;

    const onStdout = (chunk: string) => {
      stdoutBuffer += chunk;
      const lines = stdoutBuffer.split("\n");
      stdoutBuffer = lines.pop() ?? "";
      for (const line of lines) {
        if (!line.trim()) {
          continue;
        }
        const payload = JSON.parse(line);
        if (payload.id !== id || !payload.error) {
          continue;
        }
        cleanup();
        resolve({
          message: payload.error.message,
          stderr: stderrBuffer,
        });
      }
    };

    const onStderr = (chunk: string) => {
      stderrBuffer += chunk;
    };

    const onError = (error: Error) => {
      cleanup();
      reject(error);
    };

    const onExit = (code: number | null) => {
      cleanup();
      reject(
        new Error(
          `mcp bridge exited with code ${code ?? -1}: ${stderrBuffer.trim()}`,
        ),
      );
    };

    const cleanup = () => {
      child.stdout.off("data", onStdout);
      child.stderr.off("data", onStderr);
      child.off("error", onError);
      child.off("exit", onExit);
    };

    child.stdout.on("data", onStdout);
    child.stderr.on("data", onStderr);
    child.on("error", onError);
    child.on("exit", onExit);
    child.stdin.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id,
        method,
        params,
      })}\n`,
      "utf8",
    );
  });
}
