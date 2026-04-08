import { repoRoot } from "./live-sample-server.mjs";
import { createMcpClient } from "./mcp-client.mjs";

const bridgeCommand = "node integrations/mcp/bridge/index.mjs";

export function createWorkflowClient({ name, version = "0.1.0" }) {
  return createMcpClient({
    cwd: repoRoot,
    bridgeCommand,
    clientInfo: {
      name,
      version,
    },
  });
}

export async function initializeWorkflowClient({
  client,
  workflowName,
  requiredTools = [],
}) {
  await client.initialize();
  const tools = await client.listTools();
  const toolNames = tools.tools.map((tool) => tool.name);
  assertRequiredTools(toolNames, requiredTools, workflowName);
  return toolNames;
}

export function assertRequiredTools(toolNames, requiredTools, workflowName) {
  const missing = requiredTools.filter((tool) => !toolNames.includes(tool));
  if (missing.length > 0) {
    throw new Error(
      `${workflowName} missing MCP tools: ${missing.sort().join(", ")}`,
    );
  }
}

export async function closeSessionQuietly(client, sessionId) {
  if (!sessionId) {
    return;
  }

  try {
    await client.callTool("tb_session_close", { sessionId });
  } catch {
    // Best-effort cleanup to avoid leaked daemon sessions after partial workflow failures.
  }
}
