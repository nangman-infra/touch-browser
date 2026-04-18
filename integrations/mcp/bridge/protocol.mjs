import { toolCatalog as generatedToolCatalog } from "../../../contracts/generated/mcp-tool-catalog.mjs";

export const protocolVersion = "2025-06-18";

export const implementation = {
  name: "touch-browser-mcp-bridge",
  title: "Touch Browser MCP Bridge",
  version: "0.1.13",
};

export const toolCatalog = generatedToolCatalog;

export function successResponse(id, result) {
  return {
    jsonrpc: "2.0",
    id,
    result,
  };
}

export function errorResponse(id, code, message) {
  return {
    jsonrpc: "2.0",
    id,
    error: {
      code,
      message,
    },
  };
}
