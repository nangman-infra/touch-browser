import { createServeRpcClient } from "../../../scripts/lib/serve-rpc-client.mjs";
import { resolveTouchBrowserServeCommand } from "../../../scripts/lib/touch-browser-command.mjs";

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
  return resolveTouchBrowserServeCommand({ cwd, env });
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
