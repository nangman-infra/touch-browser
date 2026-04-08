import readline from "node:readline";

import {
  errorResponse,
  implementation,
  protocolVersion,
  successResponse,
  toolCatalog,
} from "./protocol.mjs";
import { createBridgeServeClient } from "./serve-client.mjs";
import { handleToolCall } from "./tool-dispatch.mjs";

const serve = createBridgeServeClient();
const input = readline.createInterface({
  input: process.stdin,
  crlfDelay: Number.POSITIVE_INFINITY,
});

let initialized = false;

input.on("line", async (line) => {
  const trimmed = line.trim();
  if (!trimmed) {
    return;
  }

  let request;
  try {
    request = JSON.parse(trimmed);
  } catch (error) {
    writeMessage(errorResponse(null, -32700, `Invalid JSON: ${String(error)}`));
    return;
  }

  try {
    const response = await handleRequest(request);
    if (response) {
      writeMessage(response);
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    writeMessage(errorResponse(request.id ?? null, -32000, message));
  }
});

input.on("close", async () => {
  await serve.close();
});

async function handleRequest(request) {
  switch (request.method) {
    case "initialize":
      await serve.ensureReady();
      initialized = true;
      return successResponse(request.id, {
        protocolVersion,
        capabilities: {
          tools: {
            listChanged: false,
          },
        },
        serverInfo: implementation,
        instructions:
          "Use the tb_* tools to drive touch-browser. Stateful browsing is available through tb_session_create, tb_search, tb_search_open_top, tb_tab_open, tb_tab_list, tb_tab_select, tb_tab_close, and tb_session_synthesize.",
      });
    case "notifications/initialized":
      return null;
    case "ping":
      return successResponse(request.id, {});
    case "tools/list":
      return successResponse(request.id, {
        tools: toolCatalog,
      });
    case "tools/call":
      if (!initialized) {
        return errorResponse(
          request.id ?? null,
          -32001,
          "MCP bridge has not been initialized.",
        );
      }
      return await handleToolCall(request.id, request.params ?? {}, serve);
    default:
      return errorResponse(
        request.id ?? null,
        -32601,
        `Unsupported MCP method: ${request.method}`,
      );
  }
}

function writeMessage(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}
