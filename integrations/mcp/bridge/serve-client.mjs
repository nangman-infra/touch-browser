import fs from "node:fs";
import path from "node:path";

import { createServeRpcClient } from "../../../scripts/lib/serve-rpc-client.mjs";

const DEFAULT_SERVE_COMMAND = "cargo run -q -p touch-browser-cli -- serve";
const PACKAGED_BINARY_CANDIDATES = [
  ["bin", "touch-browser"],
  ["dist", "touch-browser"],
  ["target", "release", "touch-browser"],
  ["target", "debug", "touch-browser"],
];

export function createBridgeServeClient({
  cwd = process.cwd(),
  serveCommand = resolveServeCommand({ cwd }),
  requestTimeoutMs = 120_000,
} = {}) {
  let client;
  let readyPromise;

  return {
    get child() {
      return currentClient().child;
    },
    async ensureReady() {
      return await withRestart(async () => {
        if (!readyPromise) {
          readyPromise = currentClient()
            .call("runtime.status", {})
            .catch((error) => {
              readyPromise = undefined;
              throw error;
            });
        }
        return await readyPromise;
      });
    },
    async call(method, params = {}) {
      await this.ensureReady();
      return await withRestart(async () => {
        return await currentClient().call(method, params);
      });
    },
    async close() {
      readyPromise = undefined;
      if (!client) {
        return;
      }
      const current = client;
      client = undefined;
      await current.close();
    },
  };

  function currentClient() {
    if (!client) {
      client = createServeRpcClient({
        cwd,
        serveCommand,
        requestTimeoutMs,
      });
    }
    return client;
  }

  async function restartClient() {
    readyPromise = undefined;
    if (client) {
      const current = client;
      client = undefined;
      await current.close().catch(() => {});
    }
    return currentClient();
  }

  async function withRestart(run) {
    try {
      return await run();
    } catch (error) {
      if (!isRestartableServeError(error)) {
        throw error;
      }

      await restartClient();
      return await run();
    }
  }
}

export function resolveServeCommand({
  cwd = process.cwd(),
  env = process.env,
} = {}) {
  const override = env.TOUCH_BROWSER_SERVE_COMMAND?.trim();
  if (override) {
    return override;
  }

  const packagedBinary = resolvePackagedBinaryPath(cwd, env);
  if (packagedBinary) {
    return `${shellEscape(packagedBinary)} serve`;
  }

  return DEFAULT_SERVE_COMMAND;
}

function isRestartableServeError(error) {
  const message = String(error?.message || error || "");
  return (
    message.includes("serve daemon exited") ||
    message.includes("stream was destroyed") ||
    message.includes("Cannot call write after a stream was destroyed") ||
    message.includes("serve RPC call timed out") ||
    message.includes("write EPIPE")
  );
}

function resolvePackagedBinaryPath(cwd, env) {
  const explicitBinary = env.TOUCH_BROWSER_SERVE_BINARY?.trim();
  if (explicitBinary && fs.existsSync(explicitBinary)) {
    return explicitBinary;
  }

  const pathMatch = resolveBinaryFromPath(env.PATH);
  if (pathMatch) {
    return pathMatch;
  }

  for (const candidate of PACKAGED_BINARY_CANDIDATES) {
    const absolutePath = path.resolve(cwd, ...candidate);
    if (fs.existsSync(absolutePath)) {
      return absolutePath;
    }
  }

  return null;
}

function resolveBinaryFromPath(pathValue) {
  if (!pathValue) {
    return null;
  }

  for (const segment of pathValue.split(path.delimiter)) {
    if (!segment) {
      continue;
    }
    const binaryPath = path.join(segment, "touch-browser");
    if (fs.existsSync(binaryPath)) {
      return binaryPath;
    }
  }

  return null;
}

function shellEscape(value) {
  return `'${String(value).replaceAll("'", `'\"'\"'`)}'`;
}
