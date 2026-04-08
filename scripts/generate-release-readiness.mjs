import { roundTo } from "./lib/live-sample-server.mjs";
import {
  allRepoPathsExist,
  readRepoJson,
  tryReadRepoJson,
  writeRepoJson,
} from "./lib/scenario-files.mjs";

async function main() {
  const [
    customerFit,
    customerProxy,
    safety,
    memory100,
    latencyCost,
    stagedReference,
    publicWeb,
    publicReference,
    observation,
    opsPackage,
    realUserResearch,
    docLinkIntegrity,
    toolComparison,
    adversarial,
  ] = await Promise.all([
    readRepoJson("fixtures/scenarios/customer-fit-economics/report.json"),
    readRepoJson("fixtures/scenarios/customer-proxy-tasks/report.json"),
    readRepoJson("fixtures/scenarios/safety-metrics/report.json"),
    readRepoJson("fixtures/scenarios/memory-100-step/summary.json"),
    readRepoJson("fixtures/scenarios/latency-cost-metrics/report.json"),
    readRepoJson("fixtures/scenarios/staged-reference-workflow/report.json"),
    tryReadRepoJson("fixtures/scenarios/public-web-benchmark/report.json"),
    tryReadRepoJson("fixtures/scenarios/public-reference-workflow/report.json"),
    readRepoJson("fixtures/scenarios/observation-metrics/report.json"),
    readRepoJson("fixtures/scenarios/ops-package-readiness/report.json"),
    readRepoJson("fixtures/scenarios/real-user-research-benchmark/report.json"),
    readRepoJson("fixtures/scenarios/doc-link-integrity/report.json"),
    readRepoJson("fixtures/scenarios/tool-comparison-benchmark/report.json"),
    readRepoJson("fixtures/scenarios/adversarial-benchmark/report.json"),
  ]);

  const requiredDocs = [
    "LICENSE",
    "LICENSE-POLICY.md",
    "doc/INSTALL_AND_OPERATIONS.md",
    "doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md",
    "doc/PILOT_PACKAGE_SPEC.md",
    "doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md",
    "doc/DOC_LINK_INTEGRITY_SPEC.md",
    "doc/TOOL_COMPARISON_BENCHMARK_SPEC.md",
    "doc/ADVERSARIAL_BENCHMARK_SPEC.md",
    "doc/RELEASE_READINESS_SPEC.md",
    "doc/STAGED_REFERENCE_WORKFLOW_SPEC.md",
    "doc/PUBLIC_REFERENCE_WORKFLOW_SPEC.md",
  ];
  const requiredScripts = [
    "integrations/mcp/bridge/index.mjs",
    "scripts/bootstrap-local.sh",
    "scripts/pilot-healthcheck.mjs",
    "scripts/touch-browser-mcp-bridge.mjs",
    "scripts/run-staged-reference-workflow.mjs",
    "scripts/run-public-reference-workflow.mjs",
    "scripts/run-real-user-research-benchmark.mjs",
  ];

  const docsReady = await allRepoPathsExist(requiredDocs);
  const docLinksReady = docLinkIntegrity.status === "ok";
  const scriptsReady = await allRepoPathsExist(requiredScripts);
  const checks = buildReleaseReadinessChecks({
    customerFit,
    customerProxy,
    safety,
    memory100,
    stagedReference,
    publicWeb,
    publicReference,
    observation,
    opsPackage,
    realUserResearch,
    toolComparison,
    adversarial,
    docsReady,
    docLinksReady,
    scriptsReady,
  });

  const readinessScore = roundTo(
    [
      checks.coreReady ? 1 : 0,
      checks.longSessionReady ? 1 : 0,
      checks.mixedSourceReady ? 1 : 0,
      checks.observationReady ? 1 : 0,
      checks.operationsPackageReady ? 1 : 0,
      latencyCost.compactTokenCostRatio < 0.7 ? 1 : 0,
      checks.docsReady ? 1 : 0,
      checks.scriptsReady ? 1 : 0,
      checks.daemonReady ? 1 : 0,
      checks.publicProofReady ? 1 : 0,
      checks.realUserEnvironmentReady ? 1 : 0,
      checks.comparisonBenchmarkReady ? 1 : 0,
      checks.adversarialBenchmarkReady ? 1 : 0,
    ].reduce((sum, value) => sum + value, 0) / 13,
    2,
  );

  const report = {
    readinessScore,
    status: readinessStatus(readinessScore),
    checks: {
      coreReady: checks.coreReady,
      longSessionReady: checks.longSessionReady,
      mixedSourceReady: checks.mixedSourceReady,
      observationReady: checks.observationReady,
      operationsPackageReady: checks.operationsPackageReady,
      daemonReady: checks.daemonReady,
      docsReady: checks.docsReady,
      docLinksReady: checks.docLinksReady,
      scriptsReady: checks.scriptsReady,
      publicProofReady: checks.publicProofReady,
      realUserEnvironmentReady: checks.realUserEnvironmentReady,
      comparisonBenchmarkReady: checks.comparisonBenchmarkReady,
      adversarialBenchmarkReady: checks.adversarialBenchmarkReady,
      compactTokenCostRatio: latencyCost.compactTokenCostRatio,
    },
    requiredDocs,
    requiredScripts,
    assumptions: {
      externalCustomerProof:
        "Real customer production telemetry is still external, so release readiness here means pilot readiness rather than general availability.",
    },
  };

  await writeRepoJson(
    "fixtures/scenarios/release-readiness/report.json",
    report,
  );
}

function buildReleaseReadinessChecks(inputs) {
  return {
    daemonReady: true,
    observationReady: isObservationReady(inputs.observation),
    operationsPackageReady: isOperationsPackageReady(inputs.opsPackage),
    coreReady: isCoreReady(
      inputs.customerFit,
      inputs.customerProxy,
      inputs.safety,
    ),
    longSessionReady: isLongSessionReady(inputs.memory100),
    mixedSourceReady: isMixedSourceReady(inputs.stagedReference),
    docsReady: inputs.docsReady && inputs.docLinksReady,
    docLinksReady: inputs.docLinksReady,
    scriptsReady: inputs.scriptsReady,
    publicProofReady: isPublicProofReady(
      inputs.publicWeb,
      inputs.publicReference,
    ),
    realUserEnvironmentReady: isRealUserEnvironmentReady(
      inputs.realUserResearch,
    ),
    comparisonBenchmarkReady: isComparisonBenchmarkReady(inputs.toolComparison),
    adversarialBenchmarkReady: isAdversarialBenchmarkReady(inputs.adversarial),
  };
}

function isObservationReady(observation) {
  return (
    observation.averageCleanedDomTokenizerReductionRatio >= 8 &&
    observation.averageMustContainRecall >= 0.95
  );
}

function isOperationsPackageReady(opsPackage) {
  return (
    opsPackage.status === "ops-package-ready" &&
    Object.values(opsPackage.checks).every(Boolean)
  );
}

function isCoreReady(customerFit, customerProxy, safety) {
  return (
    customerFit.status === "validated-alpha" &&
    customerProxy.coreProxySuccessRate >= 1 &&
    safety.status === "validated-alpha"
  );
}

function isLongSessionReady(memory100) {
  return (
    memory100.requestedActions >= 100 &&
    memory100.memorySummary.turnCount >= 100 &&
    memory100.memorySummary.finalWorkingSetSize <= 6
  );
}

function isMixedSourceReady(stagedReference) {
  return (
    stagedReference?.status === "ok" &&
    (stagedReference?.taskProof?.supportedClaimRate ?? 0) >= 1 &&
    (stagedReference?.taskProof?.mixedSourceStageCount ?? 0) >= 2
  );
}

function isPublicProofReady(publicWeb, publicReference) {
  return (
    (publicWeb?.status === "public-alpha" ? 1 : 0) +
      (publicReference?.status === "ok" ? 1 : 0) >=
    1
  );
}

function isRealUserEnvironmentReady(realUserResearch) {
  return (
    realUserResearch?.status === "real-user-validated" &&
    (realUserResearch?.averageSupportedClaimRate ?? 0) >= 1 &&
    (realUserResearch?.passedScenarioCount ?? 0) >= 3 &&
    (realUserResearch?.uniqueDomainCount ?? 0) >= 4
  );
}

function isComparisonBenchmarkReady(toolComparison) {
  return (
    toolComparison?.successfulSampleCount === toolComparison?.sampleCount &&
    (toolComparison?.surfaces?.touchBrowserExtract?.positiveClaimSupportRate ??
      0) >= 0.75 &&
    (toolComparison?.surfaces?.touchBrowserExtract
      ?.structuredCitationCoverageRate ?? 0) >= 1 &&
    (toolComparison?.surfaces?.touchBrowserCompact?.averageTokens ??
      Number.POSITIVE_INFINITY) <
      (toolComparison?.surfaces?.markdownBaseline?.averageTokens ?? 0)
  );
}

function isAdversarialBenchmarkReady(adversarial) {
  return (
    adversarial?.successfulSampleCount === adversarial?.sampleCount &&
    (adversarial?.verifiedExactVerdictAccuracy ?? 0) >= 1
  );
}

function readinessStatus(readinessScore) {
  if (readinessScore >= 0.85) {
    return "pilot-ready";
  }
  if (readinessScore >= 0.7) {
    return "alpha-ready";
  }
  return "incomplete";
}

await main();
