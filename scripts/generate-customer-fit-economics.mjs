import {
  readRepoJson,
  tryReadRepoJson,
  writeRepoJson,
} from "./lib/scenario-files.mjs";

async function main() {
  const observation = await readRepoJson(
    "fixtures/scenarios/observation-metrics/report.json",
  );
  const citation = await readRepoJson(
    "fixtures/scenarios/citation-metrics/report.json",
  );
  const liveObservation = await readRepoJson(
    "fixtures/scenarios/live-observation-metrics/report.json",
  );
  const actionSurface = await readRepoJson(
    "fixtures/scenarios/action-surface-regression/report.json",
  );
  const latencyCost = await readRepoJson(
    "fixtures/scenarios/latency-cost-metrics/report.json",
  );
  const sessionSynthesis = await readRepoJson(
    "fixtures/scenarios/live-session-synthesis/report.json",
  );
  const stagedReference = await readRepoJson(
    "fixtures/scenarios/staged-reference-workflow/report.json",
  );
  const publicWeb = await tryReadRepoJson(
    "fixtures/scenarios/public-web-benchmark/report.json",
  );
  const realUserResearch = await tryReadRepoJson(
    "fixtures/scenarios/real-user-research-benchmark/report.json",
  );

  const qualityScore = scoreAverage([
    observation.averageMustContainRecall,
    citation.averageCitationPrecision,
    citation.averageCitationRecall,
    liveObservation.averageRuntimeMustContainRecall,
    liveObservation.averageBrowserMustContainRecall,
    stagedReference?.taskProof?.supportedClaimRate ?? 0,
    publicWeb?.averageRuntimeMustContainRecall ?? 0,
    publicWeb?.averageBrowserMustContainRecall ?? 0,
    publicWeb?.taskProof?.supportedClaimRate ?? 0,
    realUserResearch?.averageSupportedClaimRate ?? 0,
  ]);
  const controlScore = scoreAverage([
    actionSurface.wrongToolOpportunityReductionRate,
    latencyCost.compactTokenCostRatio < 1 ? 1 : 0,
    sessionSynthesis.synthesis.report.visitedUrls.length >= 3 ? 1 : 0,
    (stagedReference?.taskProof?.mixedSourceStageCount ?? 0) >= 2 ? 1 : 0,
    (stagedReference?.taskProof?.listedTabCount ?? 0) >= 2 ? 1 : 0,
    publicWeb?.synthesis?.status === "ok" ? 1 : 0,
    (publicWeb?.taskProof?.supportedClaimCount ?? 0) >= 2 ? 1 : 0,
    realUserResearch?.status === "real-user-validated" ? 1 : 0,
    (realUserResearch?.uniqueDomainCount ?? 0) >= 4 ? 1 : 0,
  ]);
  const economicsScore = scoreAverage([
    normalizeGreaterThan(observation.averageHtmlTokenizerReductionRatio, 2),
    normalizeGreaterThan(
      liveObservation.averageRuntimeHtmlTokenizerReductionRatio,
      2,
    ),
    normalizeLessThan(latencyCost.compactTokenCostRatio, 0.7),
    publicWeb
      ? normalizeGreaterThan(
          publicWeb.averageRuntimeHtmlTokenizerReductionRatio,
          2,
        )
      : 0,
    publicWeb
      ? normalizeGreaterThan(
          publicWeb.averageRuntimeCleanedDomTokenizerReductionRatio,
          4,
        )
      : 0,
    realUserResearch
      ? normalizeGreaterThan(realUserResearch.averageSupportedClaimRate, 0.8)
      : 0,
  ]);

  const report = {
    wedge: "Research Agent Platform Teams",
    qualityScore,
    controlScore,
    economicsScore,
    status:
      qualityScore >= 0.9 && controlScore >= 0.7 && economicsScore >= 0.6
        ? "validated-alpha"
        : "conditional",
    assumptions: {
      economics:
        "Normalized against internal token and latency baselines rather than external vendor pricing.",
      customerFit:
        "Judged against research-agent JTBD: grounded extraction, replay, trusted-domain control, and multi-page synthesis across fixture, local live, public-web, and real-user public research benchmarks.",
    },
    evidence: {
      observationFixtureCount: observation.fixtureCount,
      citationFixtureCount: citation.fixtureCount,
      liveFixtureCount: liveObservation.fixtureCount,
      stagedWorkflowClaimCount:
        stagedReference?.taskProof?.extractedClaimCount ?? 0,
      stagedWorkflowSupportedClaimCount:
        stagedReference?.taskProof?.supportedClaimCount ?? 0,
      stagedWorkflowStageCount:
        stagedReference?.taskProof?.mixedSourceStageCount ?? 0,
      publicWebSampleCount: publicWeb?.successfulSampleCount ?? 0,
      publicWebTaskClaimCount: publicWeb?.taskProof?.extractedClaimCount ?? 0,
      publicWebTaskSupportedClaimCount:
        publicWeb?.taskProof?.supportedClaimCount ?? 0,
      realUserScenarioCount: realUserResearch?.scenarioCount ?? 0,
      realUserPassedScenarioCount: realUserResearch?.passedScenarioCount ?? 0,
      realUserSupportedClaimCount:
        realUserResearch?.totalSupportedClaimCount ?? 0,
      realUserUniqueDomainCount: realUserResearch?.uniqueDomainCount ?? 0,
      sessionVisitedUrls: sessionSynthesis.synthesis.report.visitedUrls.length,
      wrongToolOpportunityReductionRate:
        actionSurface.wrongToolOpportunityReductionRate,
    },
  };

  await writeRepoJson(
    "fixtures/scenarios/customer-fit-economics/report.json",
    report,
  );
}

function scoreAverage(values) {
  return roundTo(
    values.reduce((sum, value) => sum + value, 0) / Math.max(values.length, 1),
    2,
  );
}

function normalizeGreaterThan(value, baseline) {
  return roundTo(Math.min(value / baseline, 1), 2);
}

function normalizeLessThan(value, threshold) {
  return roundTo(Math.min(threshold / Math.max(value, 0.01), 1), 2);
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
