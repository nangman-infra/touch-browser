import { createServeRpcClient } from "./lib/serve-rpc-client.mjs";
import { resolveTouchBrowserServeCommand } from "./lib/touch-browser-command.mjs";

const serveCommand =
  process.env.TOUCH_BROWSER_HEALTHCHECK_COMMAND?.trim() ||
  resolveTouchBrowserServeCommand();
const requestTimeoutMs = Number(
  process.env.TOUCH_BROWSER_HEALTHCHECK_TIMEOUT_MS ?? "5000",
);

async function main() {
  const client = createServeRpcClient({
    serveCommand,
    requestTimeoutMs: Number.isFinite(requestTimeoutMs)
      ? requestTimeoutMs
      : 5_000,
  });

  try {
    const status = await client.call("runtime.status", {});
    console.log(
      JSON.stringify(
        {
          status: "ok",
          runtimeStatus: status.status,
          version: status.version,
          methodCount: Array.isArray(status.methods)
            ? status.methods.length
            : 0,
        },
        null,
        2,
      ),
    );
  } finally {
    await client.close();
  }
}

await main();
