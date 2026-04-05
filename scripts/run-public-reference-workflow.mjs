import { claimOutcomeForStatement } from "./lib/evidence-report.mjs";
import {
  closeSessionQuietly,
  createWorkflowClient,
  initializeWorkflowClient,
} from "./lib/reference-workflow.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";
const reportPath = "fixtures/scenarios/public-reference-workflow/report.json";

const workflowTargets = [
  {
    id: "reserved-domains",
    target: "https://www.iana.org/domains/reserved",
    allowDomain: "www.iana.org",
  },
  {
    id: "example-domains",
    target: "https://www.iana.org/domains/example",
    allowDomain: "www.iana.org",
    claim:
      "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes.",
  },
  {
    id: "robots-exclusion",
    target: "https://www.rfc-editor.org/rfc/rfc9309.html",
    allowDomain: "www.rfc-editor.org",
    claim: "RFC 9309 specifies the Robots Exclusion Protocol.",
  },
  {
    id: "reserved-top-level-names",
    target: "https://www.rfc-editor.org/rfc/rfc2606.html",
    allowDomain: "www.rfc-editor.org",
    claim: "RFC 2606 is titled Reserved Top Level DNS Names.",
  },
  {
    id: "special-use-domains",
    target: "https://www.rfc-editor.org/rfc/rfc6761.html",
    allowDomain: "www.rfc-editor.org",
    claim: "RFC 6761 is titled Special-Use Domain Names.",
  },
];

async function main() {
  const client = createWorkflowClient({
    name: "touch-browser-public-reference-workflow",
  });
  let sessionId = null;

  try {
    const toolNames = await initializeWorkflowClient({
      client,
      workflowName: "public reference workflow",
      requiredTools: [
        "tb_session_create",
        "tb_open",
        "tb_tab_open",
        "tb_extract",
        "tb_session_synthesize",
        "tb_session_close",
      ],
    });

    const created = await client.callTool("tb_session_create", {
      allowDomains: [
        ...new Set(workflowTargets.map((entry) => entry.allowDomain)),
      ],
    });
    sessionId = created.sessionId;

    const openedTabs = [];
    const extracts = [];

    for (let index = 0; index < workflowTargets.length; index += 1) {
      const entry = workflowTargets[index];
      const openResult =
        index === 0
          ? await client.callTool("tb_open", {
              sessionId,
              tabId: created.activeTabId,
              target: entry.target,
            })
          : await client.callTool("tb_tab_open", {
              sessionId,
              target: entry.target,
            });

      const tabId = index === 0 ? created.activeTabId : openResult.tabId;
      openedTabs.push({
        id: entry.id,
        target: entry.target,
        tabId,
        openResult,
      });

      if (!entry.claim) {
        continue;
      }

      const extract = await client.callTool("tb_extract", {
        sessionId,
        tabId,
        claims: [entry.claim],
      });

      extracts.push({
        id: entry.id,
        tabId,
        claim: entry.claim,
        extract,
      });
    }

    const synthesis = await client.callTool("tb_session_synthesize", {
      sessionId,
      noteLimit: 10,
    });
    const closed = await client.callTool("tb_session_close", {
      sessionId,
    });

    const taskProof = summarizeTaskProof(extracts, synthesis);
    const report = {
      status: "ok",
      question:
        "Which public sources should a research agent rely on for documentation-only domains and the robots exclusion protocol?",
      tools: toolNames,
      sessionId,
      openedTabs,
      extracts,
      synthesis,
      taskProof,
      closed,
    };

    await writeRepoJson(reportPath, report);
  } finally {
    await closeSessionQuietly(client, sessionId);
    await client.close();
  }
}

function summarizeTaskProof(extracts, synthesis) {
  const normalized = extracts.map((entry) => {
    const evidenceSupportedClaims =
      entry.extract?.result?.extract?.output?.evidenceSupportedClaims ?? [];
    const matchedOutcome = claimOutcomeForStatement(
      entry.extract?.result?.extract?.output ?? {},
      entry.claim,
    );
    const matchedSupportedClaim = evidenceSupportedClaims.find(
      (claim) => claim.statement === entry.claim,
    );

    return {
      id: entry.id,
      tabId: entry.tabId,
      claim: entry.claim,
      status: matchedSupportedClaim
        ? "supported"
        : matchedOutcome
          ? matchedOutcome.verdict
          : "unknown",
      citationCount: matchedSupportedClaim?.citations
        ? matchedSupportedClaim.citations.length
        : matchedSupportedClaim?.citation
          ? 1
          : 0,
      supportRefCount: Array.isArray(matchedSupportedClaim?.supportRefs)
        ? matchedSupportedClaim.supportRefs.length
        : Array.isArray(matchedSupportedClaim?.support)
          ? matchedSupportedClaim.support.length
          : 0,
    };
  });

  const supportedClaimCount = normalized.filter(
    (entry) => entry.status === "supported",
  ).length;

  return {
    extractedClaimCount: normalized.length,
    supportedClaimCount,
    unsupportedClaimCount: normalized.filter(
      (entry) => entry.status === "unsupported",
    ).length,
    supportedClaimRate: roundTo(
      supportedClaimCount / Math.max(normalized.length, 1),
      2,
    ),
    synthesizedNoteCount: Array.isArray(synthesis?.report?.synthesizedNotes)
      ? synthesis.report.synthesizedNotes.length
      : 0,
    extractedSamples: normalized,
  };
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
