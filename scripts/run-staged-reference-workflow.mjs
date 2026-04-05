import { renderCompactSnapshot } from "./lib/compact-snapshot.mjs";
import { claimOutcomes as getClaimOutcomes } from "./lib/evidence-report.mjs";
import { repoRoot, withLiveSampleServer } from "./lib/live-sample-server.mjs";
import {
  closeSessionQuietly,
  createWorkflowClient,
  initializeWorkflowClient,
} from "./lib/reference-workflow.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath = "fixtures/scenarios/staged-reference-workflow/report.json";

async function main() {
  const report = await withLiveSampleServer(async ({ baseUrl }) => {
    const client = createWorkflowClient({
      name: "touch-browser-staged-reference-workflow",
    });
    let sessionId = null;

    try {
      const toolNames = await initializeWorkflowClient({
        client,
        workflowName: "staged reference workflow",
        requiredTools: [
          "tb_session_create",
          "tb_open",
          "tb_extract",
          "tb_tab_open",
          "tb_tab_list",
          "tb_tab_select",
          "tb_tab_close",
          "tb_session_synthesize",
          "tb_session_close",
        ],
      });

      const created = await client.callTool("tb_session_create", {
        allowDomains: ["127.0.0.1"],
      });
      sessionId = created.sessionId;
      const publicTarget = `${baseUrl}/pricing`;
      const trustedTarget = "fixture://research/static-docs/trusted-sources";

      const publicOpen = await client.callTool("tb_open", {
        sessionId,
        tabId: created.activeTabId,
        target: publicTarget,
      });
      const publicExtract = await client.callTool("tb_extract", {
        sessionId,
        tabId: created.activeTabId,
        claims: ["Starter plan costs $29 per month."],
      });

      const trustedOpen = await client.callTool("tb_tab_open", {
        sessionId,
        target: trustedTarget,
      });
      const trustedExtract = await client.callTool("tb_extract", {
        sessionId,
        tabId: trustedOpen.tabId,
        claims: [
          "Trusted Sources recommends domain allowlists before enabling browser actions.",
        ],
      });

      const listedTabs = await client.callTool("tb_tab_list", {
        sessionId,
      });
      const selectedTrustedTab = await client.callTool("tb_tab_select", {
        sessionId,
        tabId: trustedOpen.tabId,
      });
      const synthesis = await client.callTool("tb_session_synthesize", {
        sessionId,
        noteLimit: 10,
      });
      const closedTab = await client.callTool("tb_tab_close", {
        sessionId,
        tabId: trustedOpen.tabId,
      });
      const closedSession = await client.callTool("tb_session_close", {
        sessionId,
      });

      return {
        status: "ok",
        question:
          "Can a research agent move from a public source to a trusted source without losing citation support or tab control?",
        tools: toolNames,
        sessionId,
        phases: [
          {
            id: "public-stage",
            kind: "local-live-public",
            tabId: created.activeTabId,
            target: publicTarget,
            allowDomains: ["127.0.0.1"],
            open: publicOpen,
            extract: publicExtract,
          },
          {
            id: "trusted-stage",
            kind: "trusted-fixture",
            tabId: trustedOpen.tabId,
            target: trustedTarget,
            open: trustedOpen,
            extract: trustedExtract,
            compactText:
              trustedOpen?.result?.compactText ??
              renderCompactSnapshot(
                trustedOpen?.result?.output ?? {
                  blocks: [],
                },
              ),
          },
        ],
        sourceBoundary: {
          publicTargets: [publicTarget],
          trustedTargets: [trustedTarget],
          publicAllowDomains: ["127.0.0.1"],
          trustedSourceTypes: ["fixture"],
        },
        listedTabs,
        selectedTrustedTab,
        synthesis,
        taskProof: summarizeTaskProof({
          extracts: [publicExtract, trustedExtract],
          synthesis,
          listedTabs,
          selectedTrustedTab,
          closedTab,
        }),
        closedTab,
        closedSession,
      };
    } finally {
      await closeSessionQuietly(client, sessionId);
      await client.close();
    }
  });

  await writeRepoJson(reportPath, report);
}

function summarizeTaskProof({
  extracts,
  synthesis,
  listedTabs,
  selectedTrustedTab,
  closedTab,
}) {
  const normalized = extracts.map((entry) => {
    const evidenceSupportedClaims =
      entry?.result?.extract?.output?.evidenceSupportedClaims ??
      entry?.extract?.output?.evidenceSupportedClaims ??
      [];
    const claimOutcomes = getClaimOutcomes(
      entry?.result?.extract?.output ?? entry?.extract?.output ?? {},
    );

    return {
      supportedClaimCount: evidenceSupportedClaims.length,
      unsupportedClaimCount: claimOutcomes.filter(
        (claim) => claim.verdict !== "evidence-supported",
      ).length,
    };
  });

  const supportedClaimCount = normalized.reduce(
    (sum, entry) => sum + entry.supportedClaimCount,
    0,
  );
  const unsupportedClaimCount = normalized.reduce(
    (sum, entry) => sum + entry.unsupportedClaimCount,
    0,
  );
  const extractedClaimCount = supportedClaimCount + unsupportedClaimCount;

  return {
    extractedClaimCount,
    supportedClaimCount,
    unsupportedClaimCount,
    supportedClaimRate: roundTo(
      supportedClaimCount / Math.max(extractedClaimCount, 1),
      2,
    ),
    synthesizedNoteCount: Array.isArray(synthesis?.report?.synthesizedNotes)
      ? synthesis.report.synthesizedNotes.length
      : 0,
    listedTabCount: Array.isArray(listedTabs?.tabs)
      ? listedTabs.tabs.length
      : 0,
    selectedActiveTabId: selectedTrustedTab?.activeTabId ?? null,
    closedTabRemainingCount: closedTab?.remainingTabCount ?? null,
    mixedSourceStageCount: 2,
  };
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
