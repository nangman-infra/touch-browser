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

  const coreTasks = [
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
    {
      id: "staged-reference-workflow",
      status:
        (stagedReference?.taskProof?.supportedClaimRate ?? 0) >= 1 &&
        (stagedReference?.taskProof?.mixedSourceStageCount ?? 0) >= 2 &&
        (stagedReference?.taskProof?.listedTabCount ?? 0) >= 2
          ? "passed"
          : "failed",
      supportedClaimRate: stagedReference?.taskProof?.supportedClaimRate ?? 0,
      extractedClaimCount: stagedReference?.taskProof?.extractedClaimCount ?? 0,
      noteCount: stagedReference?.taskProof?.synthesizedNoteCount ?? 0,
    },
    {
      id: "customer-fit-baseline",
      status: customerFit?.status === "validated-alpha" ? "passed" : "failed",
      supportedClaimRate: customerFit?.qualityScore ?? 0,
      extractedClaimCount: customerFit?.evidence?.publicWebTaskClaimCount ?? 0,
      noteCount: customerFit?.evidence?.sessionVisitedUrls ?? 0,
    },
    realUserResearch
      ? {
          id: "real-user-research-benchmark",
          status:
            realUserResearch?.status === "real-user-validated" &&
            (realUserResearch?.averageSupportedClaimRate ?? 0) >= 1 &&
            (realUserResearch?.passedScenarioCount ?? 0) >= 3
              ? "passed"
              : "failed",
          supportedClaimRate: realUserResearch?.averageSupportedClaimRate ?? 0,
          extractedClaimCount: realUserResearch?.totalExtractedClaimCount ?? 0,
          noteCount:
            realUserResearch?.scenarios?.reduce(
              (sum, scenario) =>
                sum + (scenario?.taskProof?.synthesizedNoteCount ?? 0),
              0,
            ) ?? 0,
        }
      : {
          id: "real-user-research-benchmark",
          status: "failed",
          supportedClaimRate: 0,
          extractedClaimCount: 0,
          noteCount: 0,
        },
  ];

  const optionalTasks = [
    publicReference
      ? {
          id: "public-reference-workflow",
          status:
            (publicReference?.taskProof?.supportedClaimRate ?? 0) >= 1 &&
            (publicReference?.taskProof?.extractedClaimCount ?? 0) >= 2
              ? "passed"
              : "failed",
          supportedClaimRate:
            publicReference?.taskProof?.supportedClaimRate ?? 0,
          extractedClaimCount:
            publicReference?.taskProof?.extractedClaimCount ?? 0,
          noteCount: publicReference?.taskProof?.synthesizedNoteCount ?? 0,
        }
      : null,
    publicWeb
      ? {
          id: "public-web-benchmark",
          status:
            publicWeb?.status === "public-alpha" &&
            (publicWeb?.taskProof?.supportedClaimRate ?? 0) >= 1
              ? "passed"
              : "failed",
          supportedClaimRate: publicWeb?.taskProof?.supportedClaimRate ?? 0,
          extractedClaimCount: publicWeb?.taskProof?.extractedClaimCount ?? 0,
          noteCount: publicWeb?.taskProof?.synthesizedNoteCount ?? 0,
        }
      : null,
  ].filter(Boolean);

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
    status:
      coreProxySuccessRate >= 1 && extendedProxySuccessRate >= 0.8
        ? "proxy-validated"
        : coreProxySuccessRate >= 1
          ? "core-proxy-validated"
          : "conditional",
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

function summarizeExtractBackedTask(synthesizedNotes, extractRecords) {
  const normalizedExtracts = extractRecords
    .map((record) => record?.result?.extract ?? record?.extract ?? null)
    .filter(Boolean);
  const extractedClaimCount = normalizedExtracts.reduce((sum, extract) => {
    const supportedClaims = extract?.output?.supportedClaims ?? [];
    const unsupportedClaims = extract?.output?.unsupportedClaims ?? [];
    return sum + supportedClaims.length + unsupportedClaims.length;
  }, 0);
  const supportedClaimCount = normalizedExtracts.reduce((sum, extract) => {
    const supportedClaims = extract?.output?.supportedClaims ?? [];
    return sum + supportedClaims.length;
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
