import { spawnShell } from "./shell-command.mjs";

export function createMcpClient({
  cwd,
  bridgeCommand,
  clientInfo,
  requestTimeoutMs = 120_000,
}) {
  const child = spawnShell(bridgeCommand, {
    cwd,
    stdio: ["pipe", "pipe", "pipe"],
  });

  const pending = new Map();
  let nextId = 0;
  let stdoutBuffer = "";
  let stderrBuffer = "";
  let fatalError = null;

  child.stdout.setEncoding("utf8");
  child.stdout.on("data", (chunk) => {
    stdoutBuffer += chunk;
    const lines = stdoutBuffer.split("\n");
    stdoutBuffer = lines.pop() ?? "";

    for (const line of lines) {
      if (!line.trim()) {
        continue;
      }

      let payload;
      try {
        payload = JSON.parse(line);
      } catch (error) {
        const parseError = new Error(
          `Invalid MCP bridge JSON response: ${line.trim()}`,
        );
        failPending(parseError);
        if (child.exitCode === null) {
          child.kill("SIGTERM");
        }
        return;
      }
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

  child.on("error", (error) => {
    failPending(error);
  });

  child.on("exit", (code) => {
    if (pending.size === 0) {
      return;
    }

    const error = new Error(
      `MCP bridge exited with code ${code ?? -1}: ${stderrBuffer.trim()}`,
    );
    for (const handler of pending.values()) {
      handler.reject(error);
    }
    pending.clear();
  });

  return {
    child,
    async initialize() {
      await call("initialize", {
        protocolVersion: "2025-06-18",
        capabilities: {},
        clientInfo,
      });
    },
    async listTools() {
      return await call("tools/list", {});
    },
    async callTool(name, args) {
      const result = await call("tools/call", {
        name,
        arguments: args,
      });
      return result.structuredContent;
    },
    async close() {
      if (child.killed || child.exitCode !== null) {
        return;
      }

      child.stdin.end();
      await new Promise((resolve) => {
        child.once("close", resolve);
        setTimeout(() => {
          if (child.exitCode === null) {
            child.kill("SIGTERM");
          }
        }, 250);
      });
    },
  };

  function call(method, params) {
    if (fatalError) {
      return Promise.reject(fatalError);
    }

    const id = ++nextId;
    const payload = JSON.stringify({
      jsonrpc: "2.0",
      id,
      method,
      params,
    });

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(id);
        reject(
          new Error(
            `MCP call timed out after ${requestTimeoutMs}ms: ${method}`,
          ),
        );
      }, requestTimeoutMs);

      pending.set(id, {
        resolve(value) {
          clearTimeout(timeout);
          resolve(value);
        },
        reject(error) {
          clearTimeout(timeout);
          reject(error);
        },
      });
      child.stdin.write(`${payload}\n`, "utf8", (error) => {
        if (error) {
          pending.delete(id);
          clearTimeout(timeout);
          reject(error);
        }
      });
    });
  }

  function failPending(error) {
    fatalError = normalizeError(error);
    for (const handler of pending.values()) {
      handler.reject(fatalError);
    }
    pending.clear();
  }
}

function normalizeError(error) {
  if (error instanceof Error) {
    return error;
  }

  return new Error(String(error));
}
