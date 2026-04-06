import {
  claimOutcomeForStatement,
  evidenceSupportedClaims,
  insufficientEvidenceClaims,
} from "./lib/evidence-report.mjs";
import {
  closeSessionQuietly,
  createWorkflowClient,
  initializeWorkflowClient,
} from "./lib/reference-workflow.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath =
  "fixtures/scenarios/real-user-research-benchmark/report.json";

const scenarios = [
  {
    id: "public-standards-research",
    question:
      "Which public standards sources should a research agent rely on for documentation-only domains and the robots exclusion protocol?",
    targets: [
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
    ],
  },
  {
    id: "public-web-api-research",
    question:
      "Which official browser API docs describe fetch and request cancellation primitives used by AI-native web tools?",
    targets: [
      {
        id: "fetch-api",
        target: "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API",
        allowDomain: "developer.mozilla.org",
        claim: "The Fetch API provides an interface for fetching resources.",
      },
      {
        id: "abort-controller",
        target:
          "https://developer.mozilla.org/en-US/docs/Web/API/AbortController",
        allowDomain: "developer.mozilla.org",
        claim:
          "The AbortController interface represents a controller object that allows you to abort one or more Web requests as and when desired.",
      },
    ],
  },
  {
    id: "public-node-runtime-research",
    question:
      "Which official Node.js docs describe path utilities and URL parsing relevant to ingestion pipelines?",
    targets: [
      {
        id: "path-module",
        target: "https://nodejs.org/api/path.html",
        allowDomain: "nodejs.org",
        claim:
          "The node:path module provides utilities for working with file and directory paths.",
      },
      {
        id: "url-module",
        target: "https://nodejs.org/api/url.html",
        allowDomain: "nodejs.org",
        claim:
          "The WHATWG URL API provides utilities for URL resolution and parsing.",
      },
    ],
  },
];

async function main() {
  const client = createWorkflowClient({
    name: "touch-browser-real-user-research-benchmark",
  });

  try {
    const toolNames = await initializeWorkflowClient({
      client,
      workflowName: "real user research benchmark",
      requiredTools: [
        "tb_session_create",
        "tb_open",
        "tb_extract",
        "tb_tab_open",
        "tb_tab_list",
        "tb_tab_select",
        "tb_session_synthesize",
        "tb_session_close",
      ],
    });

    const scenarioReports = [];
    for (const scenario of scenarios) {
      scenarioReports.push(await runScenario(client, scenario));
    }

    const passedScenarioCount = scenarioReports.filter(
      (scenario) => scenario.status === "passed",
    ).length;
    const totalExtractedClaimCount = scenarioReports.reduce(
      (sum, scenario) => sum + scenario.taskProof.extractedClaimCount,
      0,
    );
    const totalSupportedClaimCount = scenarioReports.reduce(
      (sum, scenario) => sum + scenario.taskProof.supportedClaimCount,
      0,
    );
    const averageSupportedClaimRate = roundTo(
      scenarioReports.reduce(
        (sum, scenario) => sum + scenario.taskProof.supportedClaimRate,
        0,
      ) / Math.max(scenarioReports.length, 1),
      2,
    );
    const averageListedTabCount = roundTo(
      scenarioReports.reduce(
        (sum, scenario) => sum + scenario.taskProof.listedTabCount,
        0,
      ) / Math.max(scenarioReports.length, 1),
      2,
    );
    const uniqueDomainCount = new Set(
      scenarios.flatMap((scenario) =>
        scenario.targets.map((target) => target.allowDomain),
      ),
    ).size;

    const report = {
      wedge: "Research Agent Platform Teams",
      status:
        passedScenarioCount === scenarioReports.length &&
        averageSupportedClaimRate >= 1 &&
        averageListedTabCount >= 2
          ? "real-user-validated"
          : "partial",
      tools: toolNames,
      scenarioCount: scenarioReports.length,
      passedScenarioCount,
      totalExtractedClaimCount,
      totalSupportedClaimCount,
      averageSupportedClaimRate,
      averageListedTabCount,
      uniqueDomainCount,
      scenarios: scenarioReports,
      assumptions: {
        userEnvironment:
          "This benchmark uses real public documentation sources and MCP-driven multi-tab workflows to approximate actual AI research usage. It is still a curated task suite, not uncontrolled consumer traffic.",
      },
    };

    await writeRepoJson(reportPath, report);
  } finally {
    await client.close();
  }
}

async function runScenario(client, scenario) {
  let sessionId = null;

  try {
    const allowDomains = [
      ...new Set(scenario.targets.map((t) => t.allowDomain)),
    ];
    const created = await client.callTool("tb_session_create", {
      allowDomains,
    });
    sessionId = created.sessionId;

    const openedTabs = [];
    const extracts = [];

    for (let index = 0; index < scenario.targets.length; index += 1) {
      const target = scenario.targets[index];
      const openResult =
        index === 0
          ? await client.callTool("tb_open", {
              sessionId,
              tabId: created.activeTabId,
              target: target.target,
            })
          : await client.callTool("tb_tab_open", {
              sessionId,
              target: target.target,
            });
      const tabId = index === 0 ? created.activeTabId : openResult.tabId;

      openedTabs.push({
        id: target.id,
        target: target.target,
        allowDomain: target.allowDomain,
        tabId,
        openResult: summarizeOpenResult(openResult),
      });
    }

    const listedTabs = await client.callTool("tb_tab_list", {
      sessionId,
    });

    for (const opened of openedTabs) {
      const target = scenario.targets.find((entry) => entry.id === opened.id);
      if (!target?.claim) {
        continue;
      }

      const selected = await client.callTool("tb_tab_select", {
        sessionId,
        tabId: opened.tabId,
      });
      const extract = await client.callTool("tb_extract", {
        sessionId,
        tabId: opened.tabId,
        claims: [target.claim],
      });

      extracts.push({
        id: target.id,
        tabId: opened.tabId,
        target: target.target,
        claim: target.claim,
        selected: summarizeTabSelection(selected),
        extract: summarizeExtractResult(extract),
      });
    }

    const synthesis = await client.callTool("tb_session_synthesize", {
      sessionId,
      noteLimit: 10,
    });
    const closed = await client.callTool("tb_session_close", {
      sessionId,
    });
    sessionId = null;

    const taskProof = summarizeTaskProof({
      scenario,
      extracts,
      listedTabs,
      synthesis,
      closed,
    });

    return {
      id: scenario.id,
      question: scenario.question,
      allowDomains,
      status:
        taskProof.supportedClaimRate >= 1 &&
        taskProof.listedTabCount >= scenario.targets.length &&
        taskProof.closed === true
          ? "passed"
          : "failed",
      sessionId: created.sessionId,
      openedTabs,
      listedTabs: summarizeListedTabs(listedTabs),
      extracts,
      synthesis: summarizeSynthesisResult(synthesis),
      closed: summarizeClosedSession(closed),
      taskProof,
    };
  } finally {
    await closeSessionQuietly(client, sessionId);
  }
}

function summarizeTaskProof({
  scenario,
  extracts,
  listedTabs,
  synthesis,
  closed,
}) {
  const normalized = extracts.map((entry) => {
    const supportedClaims = evidenceSupportedClaims(entry.extract);
    const unresolvedClaims = insufficientEvidenceClaims(entry.extract);
    const matchedOutcome = claimOutcomeForStatement(entry.extract, entry.claim);
    const matchedSupportedClaim = supportedClaims.find(
      (claim) => claim.statement === entry.claim,
    );
    const matchedUnsupportedClaim = unresolvedClaims.find(
      (claim) => claim.statement === entry.claim,
    );

    return {
      id: entry.id,
      target: entry.target,
      tabId: entry.tabId,
      status: researchClaimStatus(
        matchedSupportedClaim,
        matchedOutcome,
        matchedUnsupportedClaim,
      ),
      citationCount: citationCountForClaim(matchedSupportedClaim),
      supportRefCount: supportRefCountForClaim(matchedSupportedClaim),
    };
  });

  const supportedClaimCount = normalized.filter(
    (entry) => entry.status === "supported",
  ).length;
  const unsupportedClaimCount = normalized.filter(
    (entry) => entry.status === "unsupported",
  ).length;
  const extractedClaimCount = normalized.length;

  return {
    extractedClaimCount,
    supportedClaimCount,
    unsupportedClaimCount,
    supportedClaimRate: roundTo(
      supportedClaimCount / Math.max(extractedClaimCount, 1),
      2,
    ),
    synthesizedNoteCount: Array.isArray(synthesis?.synthesizedNotes)
      ? synthesis.synthesizedNotes.length
      : 0,
    listedTabCount: Array.isArray(listedTabs?.tabs)
      ? listedTabs.tabs.length
      : 0,
    expectedTabCount: scenario.targets.length,
    uniqueDomainCount: new Set(scenario.targets.map((t) => t.allowDomain)).size,
    closed: closed?.removed === true,
    extractedSamples: normalized,
  };
}

function researchClaimStatus(
  matchedSupportedClaim,
  matchedOutcome,
  matchedUnsupportedClaim,
) {
  if (matchedSupportedClaim) {
    return "supported";
  }
  if (matchedOutcome) {
    return matchedOutcome.verdict;
  }
  if (matchedUnsupportedClaim) {
    return "insufficient-evidence";
  }
  return "unknown";
}

function citationCountForClaim(matchedSupportedClaim) {
  if (matchedSupportedClaim?.citations) {
    return matchedSupportedClaim.citations.length;
  }
  return matchedSupportedClaim?.citation ? 1 : 0;
}

function supportRefCountForClaim(matchedSupportedClaim) {
  if (Array.isArray(matchedSupportedClaim?.supportRefs)) {
    return matchedSupportedClaim.supportRefs.length;
  }
  if (Array.isArray(matchedSupportedClaim?.support)) {
    return matchedSupportedClaim.support.length;
  }
  return 0;
}

function summarizeOpenResult(openResult) {
  const result = openResult?.result ?? {};
  const output = result.output ?? {};
  const budget = output.budget ?? {};

  return {
    tabId: openResult?.tabId ?? null,
    status: result.status ?? null,
    sourceUrl: output?.source?.sourceUrl ?? null,
    title: output?.source?.title ?? null,
    blockCount: Array.isArray(output?.blocks) ? output.blocks.length : 0,
    emittedTokens: budget.emittedTokens ?? null,
    truncated: budget.truncated ?? null,
  };
}

function summarizeTabSelection(selected) {
  return {
    activeTabId: selected?.activeTabId ?? null,
  };
}

function summarizeExtractResult(extract) {
  const report = extract?.result?.extract?.output ?? {};

  return {
    status: extract?.result?.status ?? null,
    sourceUrl: report?.source?.sourceUrl ?? null,
    evidenceSupportedClaims: evidenceSupportedClaims(report),
    insufficientEvidenceClaims: insufficientEvidenceClaims(report),
    contradictedClaims: report.contradictedClaims ?? [],
    needsMoreBrowsingClaims: report.needsMoreBrowsingClaims ?? [],
    claimOutcomes: report.claimOutcomes ?? [],
  };
}

function summarizeListedTabs(listedTabs) {
  return {
    activeTabId: listedTabs?.activeTabId ?? null,
    tabs: Array.isArray(listedTabs?.tabs)
      ? listedTabs.tabs.map((tab) => ({
          tabId: tab.tabId,
          currentUrl: tab.currentUrl,
          sourceType: tab.sourceType,
        }))
      : [],
  };
}

function summarizeSynthesisResult(synthesis) {
  return {
    activeTabId: synthesis?.activeTabId ?? null,
    tabCount: synthesis?.tabCount ?? null,
    synthesizedNotes: synthesis?.report?.synthesizedNotes ?? [],
    visitedUrls: synthesis?.report?.visitedUrls ?? [],
    evidenceSupportedClaims: synthesis?.report?.evidenceSupportedClaims ?? [],
    insufficientEvidenceClaims:
      synthesis?.report?.insufficientEvidenceClaims ?? [],
    contradictedClaims: synthesis?.report?.contradictedClaims ?? [],
    needsMoreBrowsingClaims: synthesis?.report?.needsMoreBrowsingClaims ?? [],
  };
}

function summarizeClosedSession(closed) {
  return {
    removed: closed?.removed ?? false,
    removedTabs: closed?.removedTabs ?? null,
    sessionId: closed?.sessionId ?? null,
  };
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
