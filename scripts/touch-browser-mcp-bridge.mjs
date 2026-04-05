import readline from "node:readline";

import { spawnShell } from "./lib/shell-command.mjs";

const protocolVersion = "2025-06-18";
const implementation = {
  name: "touch-browser-mcp-bridge",
  title: "Touch Browser MCP Bridge",
  version: "0.1.0",
};

const toolCatalog = [
  {
    name: "tb_status",
    title: "Touch Browser Status",
    description: "Return runtime and daemon capability status.",
    inputSchema: {
      type: "object",
      properties: {},
    },
  },
  {
    name: "tb_session_create",
    title: "Create Browser Session",
    description:
      "Create a long-lived touch-browser daemon session with an active tab.",
    inputSchema: {
      type: "object",
      properties: {
        headed: { type: "boolean" },
        allowDomains: {
          type: "array",
          items: { type: "string" },
        },
      },
    },
  },
  {
    name: "tb_open",
    title: "Open Target",
    description:
      "Open a target either statelessly or inside a daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        target: { type: "string" },
        browser: { type: "boolean" },
        headed: { type: "boolean" },
        sourceRisk: { type: "string" },
        sourceLabel: { type: "string" },
        verifierCommand: { type: "string" },
        allowDomains: {
          type: "array",
          items: { type: "string" },
        },
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
      required: ["target"],
    },
  },
  {
    name: "tb_search",
    title: "Search The Web",
    description:
      "Run a Google or Brave search inside touch-browser and structure the search results for follow-up browsing.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        query: { type: "string" },
        engine: { type: "string" },
        headed: { type: "boolean" },
        budget: { type: "number" },
      },
      required: ["sessionId", "query"],
    },
  },
  {
    name: "tb_search_open_result",
    title: "Open One Search Result",
    description:
      "Open one structured search result into a new tab within the daemon session.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        rank: { type: "number" },
        headed: { type: "boolean" },
      },
      required: ["sessionId", "rank"],
    },
  },
  {
    name: "tb_search_open_top",
    title: "Open Top Search Results",
    description:
      "Open the top recommended search results into new tabs for multi-page research.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        limit: { type: "number" },
        headed: { type: "boolean" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_extract",
    title: "Extract Evidence",
    description:
      "Extract evidence-supported and insufficient-evidence claims from the current target or daemon tab.",
    inputSchema: {
      type: "object",
      properties: {
        target: { type: "string" },
        claims: {
          type: "array",
          items: { type: "string" },
        },
        browser: { type: "boolean" },
        headed: { type: "boolean" },
        mainOnly: { type: "boolean" },
        verifierCommand: { type: "string" },
        sourceRisk: { type: "string" },
        sourceLabel: { type: "string" },
        allowDomains: {
          type: "array",
          items: { type: "string" },
        },
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
      required: ["claims"],
    },
  },
  {
    name: "tb_read_view",
    title: "Read View",
    description:
      "Return a readable Markdown view of a target or daemon tab for higher-level verification.",
    inputSchema: {
      type: "object",
      properties: {
        target: { type: "string" },
        browser: { type: "boolean" },
        headed: { type: "boolean" },
        mainOnly: { type: "boolean" },
        sourceRisk: { type: "string" },
        sourceLabel: { type: "string" },
        allowDomains: {
          type: "array",
          items: { type: "string" },
        },
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
    },
  },
  {
    name: "tb_policy",
    title: "Policy Report",
    description: "Return the policy evaluation for a target or daemon tab.",
    inputSchema: {
      type: "object",
      properties: {
        target: { type: "string" },
        browser: { type: "boolean" },
        headed: { type: "boolean" },
        sourceRisk: { type: "string" },
        sourceLabel: { type: "string" },
        allowDomains: {
          type: "array",
          items: { type: "string" },
        },
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
    },
  },
  {
    name: "tb_tab_open",
    title: "Open New Tab",
    description:
      "Create a new daemon tab and optionally open a target into it.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        target: { type: "string" },
        headed: { type: "boolean" },
        sourceRisk: { type: "string" },
        sourceLabel: { type: "string" },
        allowDomains: {
          type: "array",
          items: { type: "string" },
        },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_tab_list",
    title: "List Session Tabs",
    description: "List all daemon tabs registered for a session.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_tab_select",
    title: "Select Active Tab",
    description: "Set the active daemon tab for a session.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
      required: ["sessionId", "tabId"],
    },
  },
  {
    name: "tb_tab_close",
    title: "Close Session Tab",
    description: "Close one daemon tab and update the active tab if needed.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
      required: ["sessionId", "tabId"],
    },
  },
  {
    name: "tb_checkpoint",
    title: "Session Checkpoint",
    description:
      "Return the current supervised checkpoint guidance for a daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_profile",
    title: "Get Session Policy Profile",
    description: "Return the active policy profile for a daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_profile_set",
    title: "Set Session Policy Profile",
    description: "Set the active policy profile for a daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        profile: { type: "string" },
      },
      required: ["sessionId", "profile"],
    },
  },
  {
    name: "tb_click",
    title: "Click Interactive Target",
    description:
      "Click an interactive target inside an existing daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        targetRef: { type: "string" },
        headed: { type: "boolean" },
        ackRisks: {
          type: "array",
          items: { type: "string" },
        },
      },
      required: ["sessionId", "targetRef"],
    },
  },
  {
    name: "tb_type",
    title: "Type Into Interactive Field",
    description:
      "Type into an interactive field inside an existing daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        targetRef: { type: "string" },
        value: { type: "string" },
        headed: { type: "boolean" },
        sensitive: { type: "boolean" },
        ackRisks: {
          type: "array",
          items: { type: "string" },
        },
      },
      required: ["sessionId", "targetRef", "value"],
    },
  },
  {
    name: "tb_approve",
    title: "Approve Supervised Risks",
    description:
      "Persist supervised approval risks for the current daemon session so repeated ack flags are not required.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        ackRisks: {
          type: "array",
          items: { type: "string" },
        },
      },
      required: ["sessionId", "ackRisks"],
    },
  },
  {
    name: "tb_secret_store",
    title: "Store Session Secret",
    description:
      "Store a sensitive value only in daemon memory for a specific target ref.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        targetRef: { type: "string" },
        value: { type: "string" },
      },
      required: ["sessionId", "targetRef", "value"],
    },
  },
  {
    name: "tb_secret_clear",
    title: "Clear Session Secret",
    description: "Clear one stored daemon secret or all secrets for a session.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        targetRef: { type: "string" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_type_secret",
    title: "Type Stored Secret",
    description:
      "Type a previously stored daemon secret into a sensitive field without persisting it to disk.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        targetRef: { type: "string" },
        headed: { type: "boolean" },
        ackRisks: {
          type: "array",
          items: { type: "string" },
        },
      },
      required: ["sessionId", "targetRef"],
    },
  },
  {
    name: "tb_submit",
    title: "Submit Interactive Form",
    description:
      "Submit a form or submit control inside an existing daemon session/tab.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        targetRef: { type: "string" },
        headed: { type: "boolean" },
        ackRisks: {
          type: "array",
          items: { type: "string" },
        },
      },
      required: ["sessionId", "targetRef"],
    },
  },
  {
    name: "tb_refresh",
    title: "Refresh Live Session",
    description:
      "Re-capture the current persistent browser page after a manual checkpoint or out-of-band change.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        tabId: { type: "string" },
        headed: { type: "boolean" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_telemetry_summary",
    title: "Pilot Telemetry Summary",
    description:
      "Return the aggregated pilot telemetry summary for the current runtime.",
    inputSchema: {
      type: "object",
      properties: {},
    },
  },
  {
    name: "tb_telemetry_recent",
    title: "Recent Pilot Telemetry",
    description:
      "Return recent pilot telemetry events for the current runtime.",
    inputSchema: {
      type: "object",
      properties: {
        limit: { type: "integer", minimum: 1 },
      },
    },
  },
  {
    name: "tb_session_synthesize",
    title: "Synthesize Session",
    description:
      "Aggregate visited tabs inside a daemon session into a citation-ready synthesis report.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
        noteLimit: { type: "integer" },
      },
      required: ["sessionId"],
    },
  },
  {
    name: "tb_session_close",
    title: "Close Session",
    description: "Close a daemon session and clean up all tab state.",
    inputSchema: {
      type: "object",
      properties: {
        sessionId: { type: "string" },
      },
      required: ["sessionId"],
    },
  },
];

const serve = createServeClient();
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
      initialized = true;
      return successResponse(request.id, {
        protocolVersion: protocolVersion,
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
      return await handleToolCall(request.id, request.params ?? {});
    default:
      return errorResponse(
        request.id ?? null,
        -32601,
        `Unsupported MCP method: ${request.method}`,
      );
  }
}

async function handleToolCall(id, params) {
  const toolName = params?.name;
  const args = params?.arguments ?? {};
  let result;

  switch (toolName) {
    case "tb_status":
      result = await serve.call("runtime.status", {});
      break;
    case "tb_session_create":
      result = await serve.call("runtime.session.create", args);
      break;
    case "tb_open":
      result = await serve.call(
        args.sessionId ? "runtime.session.open" : "runtime.open",
        args,
      );
      break;
    case "tb_search":
      result = await serve.call("runtime.search", args);
      break;
    case "tb_search_open_result":
      result = await serve.call("runtime.search.openResult", args);
      break;
    case "tb_search_open_top":
      result = await serve.call("runtime.search.openTop", args);
      break;
    case "tb_extract":
      result = await serve.call(
        args.sessionId ? "runtime.session.extract" : "runtime.extract",
        args,
      );
      break;
    case "tb_read_view":
      result = await serve.call(
        args.sessionId ? "runtime.session.readView" : "runtime.readView",
        args,
      );
      break;
    case "tb_policy":
      result = await serve.call(
        args.sessionId ? "runtime.session.policy" : "runtime.policy",
        args,
      );
      break;
    case "tb_tab_open":
      result = await serve.call("runtime.tab.open", args);
      break;
    case "tb_tab_list":
      result = await serve.call("runtime.tab.list", args);
      break;
    case "tb_tab_select":
      result = await serve.call("runtime.tab.select", args);
      break;
    case "tb_tab_close":
      result = await serve.call("runtime.tab.close", args);
      break;
    case "tb_checkpoint":
      result = await serve.call("runtime.session.checkpoint", args);
      break;
    case "tb_profile":
      result = await serve.call("runtime.session.profile.get", args);
      break;
    case "tb_profile_set":
      result = await serve.call("runtime.session.profile.set", args);
      break;
    case "tb_click":
      result = await serve.call("runtime.session.click", args);
      break;
    case "tb_type":
      result = await serve.call("runtime.session.type", args);
      break;
    case "tb_approve":
      result = await serve.call("runtime.session.approve", args);
      break;
    case "tb_secret_store":
      result = await serve.call("runtime.session.secret.store", args);
      break;
    case "tb_secret_clear":
      result = await serve.call("runtime.session.secret.clear", args);
      break;
    case "tb_type_secret":
      result = await serve.call("runtime.session.typeSecret", args);
      break;
    case "tb_submit":
      result = await serve.call("runtime.session.submit", args);
      break;
    case "tb_refresh":
      result = await serve.call("runtime.session.refresh", args);
      break;
    case "tb_telemetry_summary":
      result = await serve.call("runtime.telemetry.summary", args);
      break;
    case "tb_telemetry_recent":
      result = await serve.call("runtime.telemetry.recent", args);
      break;
    case "tb_session_synthesize":
      result = await serve.call("runtime.session.synthesize", args);
      break;
    case "tb_session_close":
      result = await serve.call("runtime.session.close", args);
      break;
    default:
      return errorResponse(id, -32602, `Unknown tool: ${toolName}`);
  }

  return successResponse(id, {
    content: [
      {
        type: "text",
        text: JSON.stringify(result, null, 2),
      },
    ],
    structuredContent: result,
    isError: false,
  });
}

function createServeClient() {
  const child = spawnShell("cargo run -q -p touch-browser-cli -- serve", {
    cwd: process.cwd(),
    env: {
      ...process.env,
      TOUCH_BROWSER_TELEMETRY_SURFACE: "mcp",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });

  const pending = new Map();
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
      `touch-browser serve exited with code ${code ?? -1}: ${stderrBuffer.trim()}`,
    );
    for (const handler of pending.values()) {
      handler.reject(error);
    }
    pending.clear();
  });

  return {
    async call(method, params = {}) {
      const id = ++nextId;
      const payload = JSON.stringify({
        jsonrpc: "2.0",
        id,
        method,
        params,
      });

      return await new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        child.stdin.write(`${payload}\n`, "utf8", (error) => {
          if (error) {
            pending.delete(id);
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
}

function successResponse(id, result) {
  return {
    jsonrpc: "2.0",
    id,
    result,
  };
}

function errorResponse(id, code, message) {
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code,
      message,
    },
  };
}

function writeMessage(payload) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}
