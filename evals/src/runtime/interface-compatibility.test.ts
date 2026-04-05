import { spawn } from "node:child_process";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";

type RpcEnvelope<T> = {
  readonly id: number | string | null;
  readonly jsonrpc: "2.0";
  readonly result: T;
};

type ServeStatusResult = {
  readonly methods: readonly string[];
  readonly status: "ready";
  readonly version: string;
};

type McpToolListResult = {
  readonly tools: readonly {
    readonly name: string;
  }[];
};

describe("interface compatibility", () => {
  it("keeps CLI JSON surfaces stable for open and compact-view, and raw Markdown stable for read-view", async () => {
    const open = JSON.parse(
      await runShell(
        "cargo run -q -p touch-browser-cli -- open fixture://research/static-docs/getting-started",
      ),
    );
    const compact = JSON.parse(
      await runShell(
        "cargo run -q -p touch-browser-cli -- compact-view fixture://research/static-docs/getting-started",
      ),
    );
    const readView = await runShell(
      "cargo run -q -p touch-browser-cli -- read-view fixture://research/static-docs/getting-started",
    );

    expect(open.version).toBe("1.0.0");
    expect(open.action).toBe("open");
    expect(open.status).toBe("succeeded");
    expect(open.output.version).toBe("1.0.0");
    expect(open.output.stableRefVersion).toBe("1");

    expect(compact.sourceUrl).toBe(
      "fixture://research/static-docs/getting-started",
    );
    expect(compact.compactText.length).toBeGreaterThan(0);
    expect(compact.lineCount).toBeGreaterThan(0);
    expect(Array.isArray(compact.refIndex)).toBe(true);
    expect(compact.refIndex.length).toBeGreaterThan(0);
    expect(readView.startsWith("# ")).toBe(true);
    expect(readView).toContain("Getting Started");
  }, 20_000);

  it("keeps serve and MCP minimal contracts stable", async () => {
    const serveStatus = await runServeCall<ServeStatusResult>(
      "runtime.status",
      {},
    );
    expect(serveStatus.status).toBe("ready");
    expect(serveStatus.methods).toContain("runtime.open");
    expect(serveStatus.methods).toContain("runtime.readView");
    expect(serveStatus.methods).toContain("runtime.search");
    expect(serveStatus.methods).toContain("runtime.search.openTop");
    expect(serveStatus.methods).toContain("runtime.session.click");
    expect(serveStatus.methods).toContain("runtime.session.type");
    expect(serveStatus.methods).toContain("runtime.session.submit");
    expect(serveStatus.methods).toContain("runtime.session.refresh");
    expect(serveStatus.methods).toContain("runtime.session.checkpoint");
    expect(serveStatus.methods).toContain("runtime.session.approve");
    expect(serveStatus.methods).toContain("runtime.session.profile.get");
    expect(serveStatus.methods).toContain("runtime.session.profile.set");
    expect(serveStatus.methods).toContain("runtime.session.typeSecret");
    expect(serveStatus.methods).toContain("runtime.session.secret.store");
    expect(serveStatus.methods).toContain("runtime.telemetry.summary");
    expect(serveStatus.methods).toContain("runtime.telemetry.recent");
    expect(serveStatus.version).toBe("1.0.0");

    const mcpToolList = await runMcpCall<McpToolListResult>("tools/list", {});
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_status",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_extract",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_search",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_search_open_top",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_read_view",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_click",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_checkpoint",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_profile",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_profile_set",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_type",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_submit",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_refresh",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_approve",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) => tool.name === "tb_secret_store",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) =>
          tool.name === "tb_telemetry_summary",
      ),
    ).toBe(true);
    expect(
      mcpToolList.tools.some(
        (tool: { readonly name: string }) =>
          tool.name === "tb_telemetry_recent",
      ),
    ).toBe(true);
  }, 20_000);
});

async function runShell(command: string) {
  const child = spawn("zsh", ["-lic", command], {
    cwd: repoRoot,
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

async function runServeCall<T>(
  method: string,
  params: Record<string, unknown>,
): Promise<T> {
  const child = spawn(
    "zsh",
    ["-lic", "cargo run -q -p touch-browser-cli -- serve"],
    {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    },
  );

  const result = await new Promise<T>((resolve, reject) => {
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
        const payload = JSON.parse(line) as RpcEnvelope<T>;
        resolve(payload.result);
        child.stdin.end();
      }
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderrBuffer += chunk;
    });

    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0 && code !== null) {
        reject(new Error(stderrBuffer.trim()));
      }
    });

    child.stdin.write(
      `${JSON.stringify({
        jsonrpc: "2.0",
        id: 1,
        method,
        params,
      })}\n`,
    );
  });

  return result;
}

async function runMcpCall<T>(
  method: string,
  params: Record<string, unknown>,
): Promise<T> {
  const child = spawn(
    "zsh",
    ["-lic", "node scripts/touch-browser-mcp-bridge.mjs"],
    {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    },
  );

  const initialize = {
    jsonrpc: "2.0",
    id: 1,
    method: "initialize",
    params: {
      protocolVersion: "2025-06-18",
      capabilities: {},
      clientInfo: { name: "vitest", version: "0.0.0" },
    },
  };
  const payload = { jsonrpc: "2.0", id: 2, method, params };

  return await new Promise<T>((resolve, reject) => {
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
        const response = JSON.parse(line) as RpcEnvelope<T>;
        if (response.id === 2) {
          resolve(response.result);
          child.stdin.end();
        }
      }
    });

    child.stderr.setEncoding("utf8");
    child.stderr.on("data", (chunk) => {
      stderrBuffer += chunk;
    });

    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0 && code !== null) {
        reject(new Error(stderrBuffer.trim()));
      }
    });

    child.stdin.write(`${JSON.stringify(initialize)}\n`);
    child.stdin.write(`${JSON.stringify(payload)}\n`);
  });
}
