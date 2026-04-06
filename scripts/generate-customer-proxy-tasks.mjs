import {
  evidenceSupportedClaims,
  claimOutcomes as getClaimOutcomes,
} from "./lib/evidence-report.mjs";
import { roundTo } from "./lib/live-sample-server.mjs";
import {
  readRepoJson,
  tryReadRepoJson,
  writeRepoJson,
} from "./lib/scenario-files.mjs";

async function main() {
  const referenceWorkflow = await readRepoJson(
    "fixtures/scenarios/reference-research-workflow/report.json",
  );
  const liveSynthesis = await readRepoJson(
    "fixtures/scenarios/live-session-synthesis/report.json",
  );
  const stagedReference = await readRepoJson(
    "fixtures/scenarios/staged-reference-workflow/report.json",
  );
  const customerFit = await readRepoJson(
    "fixtures/scenarios/customer-fit-economics/report.json",
  );
  const publicReference = await tryReadRepoJson(
    "fixtures/scenarios/public-reference-workflow/report.json",
  );
  const publicWeb = await tryReadRepoJson(
    "fixtures/scenarios/public-web-benchmark/report.json",
  );
  const realUserResearch = await tryReadRepoJson(
    "fixtures/scenarios/real-user-research-benchmark/report.json",
  );

  const coreTasks = buildCoreTasks({
    referenceWorkflow,
    liveSynthesis,
    stagedReference,
    customerFit,
    realUserResearch,
  });
  const optionalTasks = buildOptionalTasks({ publicReference, publicWeb });

  const corePassed = coreTasks.filter(
    (task) => task.status === "passed",
  ).length;
  const optionalPassed = optionalTasks.filter(
    (task) => task.status === "passed",
  ).length;

  const coreProxySuccessRate = roundTo(
    corePassed / Math.max(coreTasks.length, 1),
    2,
  );
  const extendedProxySuccessRate = roundTo(
    (corePassed + optionalPassed) /
      Math.max(coreTasks.length + optionalTasks.length, 1),
    2,
  );

  const report = {
    wedge: "Research Agent Platform Teams",
    coreTaskCount: coreTasks.length,
    optionalTaskCount: optionalTasks.length,
    coreProxySuccessRate,
    extendedProxySuccessRate,
    status: proxyStatus(coreProxySuccessRate, extendedProxySuccessRate),
    assumptions: {
      realCustomer:
        "External production telemetry is not available in-repo, so proxy tasks use fixture, local live, public-web, and MCP-backed workflows as a substitute gate.",
    },
    tasks: [...coreTasks, ...optionalTasks],
  };

  await writeRepoJson(
    "fixtures/scenarios/customer-proxy-tasks/report.json",
    report,
  );
}

function buildCoreTasks({
  referenceWorkflow,
  liveSynthesis,
  stagedReference,
  customerFit,
  realUserResearch,
}) {
  return [
    {
      id: "fixture-reference-workflow",
      ...summarizeExtractBackedTask(
        referenceWorkflow?.synthesis?.report?.synthesizedNotes ?? [],
        [referenceWorkflow?.pricingExtract, referenceWorkflow?.docsExtract],
      ),
    },
    {
      id: "local-live-session-synthesis",
      ...summarizeExtractBackedTask(
        liveSynthesis?.synthesis?.report?.synthesizedNotes ?? [],
        [liveSynthesis?.docsExtract, liveSynthesis?.pricingExtract],
      ),
    },
    stagedReferenceTask(stagedReference),
    customerFitTask(customerFit),
    realUserResearchTask(realUserResearch),
  ];
}

function buildOptionalTasks({ publicReference, publicWeb }) {
  return [
    publicReferenceTask(publicReference),
    publicWebTask(publicWeb),
  ].filter(Boolean);
}

function stagedReferenceTask(stagedReference) {
  const passed =
    (stagedReference?.taskProof?.supportedClaimRate ?? 0) >= 1 &&
    (stagedReference?.taskProof?.mixedSourceStageCount ?? 0) >= 2 &&
    (stagedReference?.taskProof?.listedTabCount ?? 0) >= 2;

  return {
    id: "staged-reference-workflow",
    status: passed ? "passed" : "failed",
    supportedClaimRate: stagedReference?.taskProof?.supportedClaimRate ?? 0,
    extractedClaimCount: stagedReference?.taskProof?.extractedClaimCount ?? 0,
    noteCount: stagedReference?.taskProof?.synthesizedNoteCount ?? 0,
  };
}

function customerFitTask(customerFit) {
  return {
    id: "customer-fit-baseline",
    status: customerFit?.status === "validated-alpha" ? "passed" : "failed",
    supportedClaimRate: customerFit?.qualityScore ?? 0,
    extractedClaimCount: customerFit?.evidence?.publicWebTaskClaimCount ?? 0,
    noteCount: customerFit?.evidence?.sessionVisitedUrls ?? 0,
  };
}

function realUserResearchTask(realUserResearch) {
  if (!realUserResearch) {
    return {
      id: "real-user-research-benchmark",
      status: "failed",
      supportedClaimRate: 0,
      extractedClaimCount: 0,
      noteCount: 0,
    };
  }

  const passed =
    realUserResearch.status === "real-user-validated" &&
    (realUserResearch.averageSupportedClaimRate ?? 0) >= 1 &&
    (realUserResearch.passedScenarioCount ?? 0) >= 3;

  return {
    id: "real-user-research-benchmark",
    status: passed ? "passed" : "failed",
    supportedClaimRate: realUserResearch.averageSupportedClaimRate ?? 0,
    extractedClaimCount: realUserResearch.totalExtractedClaimCount ?? 0,
    noteCount:
      realUserResearch.scenarios?.reduce(
        (sum, scenario) =>
          sum + (scenario?.taskProof?.synthesizedNoteCount ?? 0),
        0,
      ) ?? 0,
  };
}

function publicReferenceTask(publicReference) {
  if (!publicReference) {
    return null;
  }

  const passed =
    (publicReference.taskProof?.supportedClaimRate ?? 0) >= 1 &&
    (publicReference.taskProof?.extractedClaimCount ?? 0) >= 2;

  return {
    id: "public-reference-workflow",
    status: passed ? "passed" : "failed",
    supportedClaimRate: publicReference.taskProof?.supportedClaimRate ?? 0,
    extractedClaimCount: publicReference.taskProof?.extractedClaimCount ?? 0,
    noteCount: publicReference.taskProof?.synthesizedNoteCount ?? 0,
  };
}

function publicWebTask(publicWeb) {
  if (!publicWeb) {
    return null;
  }

  const passed =
    publicWeb.status === "public-alpha" &&
    (publicWeb.taskProof?.supportedClaimRate ?? 0) >= 1;

  return {
    id: "public-web-benchmark",
    status: passed ? "passed" : "failed",
    supportedClaimRate: publicWeb.taskProof?.supportedClaimRate ?? 0,
    extractedClaimCount: publicWeb.taskProof?.extractedClaimCount ?? 0,
    noteCount: publicWeb.taskProof?.synthesizedNoteCount ?? 0,
  };
}

function proxyStatus(coreProxySuccessRate, extendedProxySuccessRate) {
  if (coreProxySuccessRate >= 1 && extendedProxySuccessRate >= 0.8) {
    return "proxy-validated";
  }
  if (coreProxySuccessRate >= 1) {
    return "core-proxy-validated";
  }
  return "conditional";
}

function summarizeExtractBackedTask(synthesizedNotes, extractRecords) {
  const normalizedExtracts = extractRecords
    .map((record) => record?.result?.extract ?? record?.extract ?? null)
    .filter(Boolean);
  const extractedClaimCount = normalizedExtracts.reduce((sum, extract) => {
    const output = extract?.output ?? {};
    const supportedClaims = evidenceSupportedClaims(output);
    const unresolvedCount = getClaimOutcomes(output).filter(
      (claim) => claim.verdict !== "evidence-supported",
    ).length;
    return sum + supportedClaims.length + unresolvedCount;
  }, 0);
  const supportedClaimCount = normalizedExtracts.reduce((sum, extract) => {
    return sum + evidenceSupportedClaims(extract?.output ?? {}).length;
  }, 0);
  const supportedClaimRate = roundTo(
    supportedClaimCount / Math.max(extractedClaimCount, 1),
    2,
  );

  return {
    status:
      normalizedExtracts.length > 0 &&
      synthesizedNotes.length > 0 &&
      supportedClaimRate >= 0.5
        ? "passed"
        : "failed",
    supportedClaimRate,
    extractedClaimCount,
    noteCount: synthesizedNotes.length,
  };
}

await main();
