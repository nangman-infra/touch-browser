import { errorResponse, successResponse } from "./protocol.mjs";

function rejectManagedMcpArguments(id, toolName, args) {
  if (!args || typeof args !== "object") {
    return null;
  }

  if ("headed" in args) {
    return errorResponse(
      id,
      -32602,
      `Tool ${toolName} does not accept \`headed\` over MCP. touch-browser MCP is headless-only; request supervised recovery instead.`,
    );
  }

  if ("engine" in args) {
    return errorResponse(
      id,
      -32602,
      `Tool ${toolName} does not accept \`engine\` over MCP. Search engine selection is automatic on the public docs/research web surface.`,
    );
  }

  return null;
}

function sanitizeMcpArgs(args) {
  if (!args || typeof args !== "object") {
    return {};
  }

  const { headed: _headed, engine: _engine, ...rest } = args;
  return rest;
}

export async function handleToolCall(id, params, serve) {
  const toolName = params?.name;
  const rawArgs = params?.arguments ?? {};
  const rejected = rejectManagedMcpArguments(id, toolName, rawArgs);
  if (rejected) {
    return rejected;
  }
  const args = sanitizeMcpArgs(rawArgs);
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
      result = await serve.call("runtime.search", {
        ...args,
        engine: undefined,
      });
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
