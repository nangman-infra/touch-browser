import { spawn } from "node:child_process";

import { repoRoot } from "./live-sample-server.mjs";

export function createServeRpcClient({
  cwd = repoRoot,
  serveCommand = "target/debug/touch-browser serve",
  requestTimeoutMs = 120_000,
} = {}) {
  const child = spawn("zsh", ["-lic", serveCommand], {
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
      } catch {
        const parseError = new Error(
          `Invalid serve daemon JSON response: ${line.trim()}`,
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
      `serve daemon exited with code ${code ?? -1}: ${stderrBuffer.trim()}`,
    );
    failPending(error);
  });

  return {
    child,
    async call(method, params = {}) {
      if (fatalError) {
        throw fatalError;
      }

      const id = ++nextId;
      const payload = JSON.stringify({
        jsonrpc: "2.0",
        id,
        method,
        params,
      });

      return await new Promise((resolve, reject) => {
        const timeout = setTimeout(() => {
          pending.delete(id);
          reject(
            new Error(
              `serve RPC call timed out after ${requestTimeoutMs}ms: ${method}`,
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
