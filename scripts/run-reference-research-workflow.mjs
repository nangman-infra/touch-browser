import {
  closeSessionQuietly,
  createWorkflowClient,
  initializeWorkflowClient,
} from "./lib/reference-workflow.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";
const reportPath = "fixtures/scenarios/reference-research-workflow/report.json";

async function main() {
  const client = createWorkflowClient({
    name: "touch-browser-reference-workflow",
  });
  let sessionId = null;

  try {
    const toolNames = await initializeWorkflowClient({
      client,
      workflowName: "reference research workflow",
      requiredTools: [
        "tb_session_create",
        "tb_open",
        "tb_extract",
        "tb_tab_open",
        "tb_session_synthesize",
        "tb_session_close",
      ],
    });

    const created = await client.callTool("tb_session_create", {});
    sessionId = created.sessionId;
    const firstTabId = created.activeTabId;

    const pricing = await client.callTool("tb_open", {
      sessionId,
      tabId: firstTabId,
      target: "fixture://research/citation-heavy/pricing",
    });
    const pricingExtract = await client.callTool("tb_extract", {
      sessionId,
      tabId: firstTabId,
      claims: ["The Starter plan costs $29 per month."],
    });

    const docs = await client.callTool("tb_tab_open", {
      sessionId,
      target: "fixture://research/static-docs/getting-started",
    });
    const docsExtract = await client.callTool("tb_extract", {
      sessionId,
      tabId: docs.tabId,
      claims: [
        "Stable references identify interactive blocks across sessions.",
      ],
    });

    const synthesis = await client.callTool("tb_session_synthesize", {
      sessionId,
      noteLimit: 8,
    });
    const closed = await client.callTool("tb_session_close", {
      sessionId,
    });

    const report = {
      status: "ok",
      tools: toolNames,
      sessionId,
      firstTabId,
      secondTabId: docs.tabId,
      pricing,
      pricingExtract,
      docs,
      docsExtract,
      synthesis,
      closed,
    };

    await writeRepoJson(reportPath, report);
  } finally {
    await closeSessionQuietly(client, sessionId);
    await client.close();
  }
}

await main();
