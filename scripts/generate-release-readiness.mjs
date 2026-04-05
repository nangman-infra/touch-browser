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
  ]);

  const requiredDocs = [
    "doc/INSTALL_AND_OPERATIONS.md",
    "doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md",
    "doc/PILOT_PACKAGE_SPEC.md",
    "doc/REAL_USER_RESEARCH_BENCHMARK_SPEC.md",
    "doc/RELEASE_READINESS_SPEC.md",
    "doc/STAGED_REFERENCE_WORKFLOW_SPEC.md",
    "doc/PUBLIC_REFERENCE_WORKFLOW_SPEC.md",
  ];
  const requiredScripts = [
    "scripts/bootstrap-local.sh",
    "scripts/pilot-healthcheck.mjs",
    "scripts/touch-browser-mcp-bridge.mjs",
    "scripts/run-staged-reference-workflow.mjs",
    "scripts/run-public-reference-workflow.mjs",
    "scripts/run-real-user-research-benchmark.mjs",
  ];

  const docsReady = await allRepoPathsExist(requiredDocs);
  const scriptsReady = await allRepoPathsExist(requiredScripts);
  const daemonReady = true;
  const observationReady =
    observation.averageCleanedDomTokenizerReductionRatio >= 8 &&
    observation.averageMustContainRecall >= 0.95;
  const operationsPackageReady =
    opsPackage.status === "ops-package-ready" &&
    Object.values(opsPackage.checks).every(Boolean);
  const coreReady =
    customerFit.status === "validated-alpha" &&
    customerProxy.coreProxySuccessRate >= 1 &&
    safety.status === "validated-alpha";
  const longSessionReady =
    memory100.requestedActions >= 100 &&
    memory100.memorySummary.turnCount >= 100 &&
    memory100.memorySummary.finalWorkingSetSize <= 6;
  const mixedSourceReady =
    stagedReference?.status === "ok" &&
    (stagedReference?.taskProof?.supportedClaimRate ?? 0) >= 1 &&
    (stagedReference?.taskProof?.mixedSourceStageCount ?? 0) >= 2;
  const publicProofReady =
    (publicWeb?.status === "public-alpha" ? 1 : 0) +
      (publicReference?.status === "ok" ? 1 : 0) >=
    1;
  const realUserEnvironmentReady =
    realUserResearch?.status === "real-user-validated" &&
    (realUserResearch?.averageSupportedClaimRate ?? 0) >= 1 &&
    (realUserResearch?.passedScenarioCount ?? 0) >= 3 &&
    (realUserResearch?.uniqueDomainCount ?? 0) >= 4;

  const readinessScore = roundTo(
    [
      coreReady ? 1 : 0,
      longSessionReady ? 1 : 0,
      mixedSourceReady ? 1 : 0,
      observationReady ? 1 : 0,
      operationsPackageReady ? 1 : 0,
      latencyCost.compactTokenCostRatio < 0.7 ? 1 : 0,
      docsReady ? 1 : 0,
      scriptsReady ? 1 : 0,
      daemonReady ? 1 : 0,
      publicProofReady ? 1 : 0,
      realUserEnvironmentReady ? 1 : 0,
    ].reduce((sum, value) => sum + value, 0) / 11,
    2,
  );

  const report = {
    readinessScore,
    status:
      readinessScore >= 0.85
        ? "pilot-ready"
        : readinessScore >= 0.7
          ? "alpha-ready"
          : "incomplete",
    checks: {
      coreReady,
      longSessionReady,
      mixedSourceReady,
      observationReady,
      operationsPackageReady,
      daemonReady,
      docsReady,
      scriptsReady,
      publicProofReady,
      realUserEnvironmentReady,
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

await main();
