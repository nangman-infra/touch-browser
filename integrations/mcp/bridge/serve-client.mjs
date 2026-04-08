import { createServeRpcClient } from "../../../scripts/lib/serve-rpc-client.mjs";

const DEFAULT_SERVE_COMMAND = "cargo run -q -p touch-browser-cli -- serve";

export function createBridgeServeClient({
  cwd = process.cwd(),
  serveCommand = resolveServeCommand(),
  requestTimeoutMs = 120_000,
} = {}) {
  const client = createServeRpcClient({
    cwd,
    serveCommand,
    requestTimeoutMs,
  });
  let readyPromise;

  return {
    child: client.child,
    async ensureReady() {
      if (!readyPromise) {
        readyPromise = client.call("runtime.status", {}).catch((error) => {
          readyPromise = undefined;
          throw error;
        });
      }
      return await readyPromise;
    },
    async call(method, params = {}) {
      await this.ensureReady();
      return await client.call(method, params);
    },
    async close() {
      await client.close();
    },
  };
}

export function resolveServeCommand() {
  return (
    process.env.TOUCH_BROWSER_SERVE_COMMAND?.trim() || DEFAULT_SERVE_COMMAND
  );
}
