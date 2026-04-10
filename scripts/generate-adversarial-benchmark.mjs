import {
  ensureCliBuilt,
  roundTo,
  runShell,
  shellEscape,
} from "./lib/live-sample-server.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath = "fixtures/scenarios/adversarial-benchmark/report.json";

const verifierCommand = "node scripts/example-verifier.mjs";

const scenarios = [
  {
    id: "lambda-welcome-needs-more-browsing",
    url: "https://docs.aws.amazon.com/lambda/latest/dg/welcome.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "The maximum timeout for a Lambda function is 15 minutes.",
    expectedVerdict: "needs-more-browsing",
    expectedMode: "review",
  },
  {
    id: "lambda-limits-supported",
    url: "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "The maximum timeout for a Lambda function is 15 minutes.",
    expectedVerdict: "evidence-supported",
    expectedMode: "auto-answer",
  },
  {
    id: "lambda-limits-review-on-false-max",
    url: "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "The maximum timeout for a Lambda function is 24 hours.",
    expectedVerdict: "needs-more-browsing",
    expectedMode: "review",
  },
  {
    id: "ecs-overview-needs-more-browsing",
    url: "https://docs.aws.amazon.com/AmazonECS/latest/developerguide/Welcome.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "ECS supports GPU instances natively.",
    expectedVerdict: "needs-more-browsing",
    expectedMode: "review",
  },
  {
    id: "iana-registration-review",
    url: "https://www.iana.org/help/example-domains",
    allowDomain: "www.iana.org",
    claim: "Example domains are available for registration.",
    expectedVerdict: "needs-more-browsing",
    expectedMode: "review",
  },
];

async function main() {
  await ensureCliBuilt();

  const scenarioReports = [];
  for (const scenario of scenarios) {
    scenarioReports.push(await evaluateScenario(scenario));
  }

  const successful = scenarioReports.filter((scenario) => !scenario.error);
  const rawCorrectCount = successful.filter(
    (scenario) => scenario.rawVerdict === scenario.expectedVerdict,
  ).length;
  const verifiedCorrectCount = successful.filter(
    (scenario) => scenario.verifiedVerdict === scenario.expectedVerdict,
  ).length;
  const rawHighBandPredictions = successful.filter(
    isUnsafeOrAutoAnswerCandidate("raw"),
  );
  const verifiedHighBandPredictions = successful.filter(
    isUnsafeOrAutoAnswerCandidate("verified"),
  );
  const reviewRequiredScenarios = successful.filter(
    (scenario) => scenario.expectedMode === "review",
  );
  const rawReviewCapturedCount = reviewRequiredScenarios.filter((scenario) =>
    reviewCaptured(scenario, "raw"),
  ).length;
  const verifiedReviewCapturedCount = reviewRequiredScenarios.filter(
    (scenario) => reviewCaptured(scenario, "verified"),
  ).length;
  const explainabilityCoveredCount = successful.filter(
    (scenario) =>
      scenario.rawHasVerdictExplanation &&
      (!isSupportedVerdict(scenario.rawVerdict) ||
        scenario.rawSupportSnippetCount > 0),
  ).length;
  const verifiedExplainabilityCoveredCount = successful.filter(
    (scenario) =>
      scenario.verifiedHasVerdictExplanation &&
      (!isSupportedVerdict(scenario.verifiedVerdict) ||
        scenario.verifiedSupportSnippetCount > 0),
  ).length;

  const report = {
    checkedAt: new Date().toISOString(),
    sampleCount: scenarios.length,
    successfulSampleCount: successful.length,
    rawExactVerdictAccuracy:
      successful.length === 0
        ? 0
        : roundTo(rawCorrectCount / successful.length, 2),
    verifiedExactVerdictAccuracy:
      successful.length === 0
        ? 0
        : roundTo(verifiedCorrectCount / successful.length, 2),
    rawHighBandPrecision:
      rawHighBandPredictions.length === 0
        ? 1
        : roundTo(
            rawHighBandPredictions.filter(
              (scenario) => scenario.expectedMode === "auto-answer",
            ).length / rawHighBandPredictions.length,
            2,
          ),
    verifiedHighBandPrecision:
      verifiedHighBandPredictions.length === 0
        ? 1
        : roundTo(
            verifiedHighBandPredictions.filter(
              (scenario) => scenario.expectedMode === "auto-answer",
            ).length / verifiedHighBandPredictions.length,
            2,
          ),
    rawUnsafeAutoAnswerCount: rawHighBandPredictions.filter(
      (scenario) => scenario.expectedMode !== "auto-answer",
    ).length,
    verifiedUnsafeAutoAnswerCount: verifiedHighBandPredictions.filter(
      (scenario) => scenario.expectedMode !== "auto-answer",
    ).length,
    rawReviewCaptureRate:
      reviewRequiredScenarios.length === 0
        ? 1
        : roundTo(rawReviewCapturedCount / reviewRequiredScenarios.length, 2),
    verifiedReviewCaptureRate:
      reviewRequiredScenarios.length === 0
        ? 1
        : roundTo(
            verifiedReviewCapturedCount / reviewRequiredScenarios.length,
            2,
          ),
    rawExplainabilityCoverage:
      successful.length === 0
        ? 0
        : roundTo(explainabilityCoveredCount / successful.length, 2),
    verifiedExplainabilityCoverage:
      successful.length === 0
        ? 0
        : roundTo(verifiedExplainabilityCoveredCount / successful.length, 2),
    verifierCommand,
    scenarios: scenarioReports,
    status:
      successful.length === scenarios.length &&
      verifiedCorrectCount === successful.length &&
      reportGateSatisfied({
        rawHighBandPredictions,
        verifiedHighBandPredictions,
        reviewRequiredScenarios,
        rawReviewCapturedCount,
        verifiedReviewCapturedCount,
        explainabilityCoveredCount,
        verifiedExplainabilityCoveredCount,
        successfulCount: successful.length,
      })
        ? "adversarial-validated"
        : "partial",
  };

  await writeRepoJson(reportPath, report);
}

async function evaluateScenario(scenario) {
  try {
    const raw = await runExtract(scenario, null);
    const verified = await runExtract(scenario, verifierCommand);

    return {
      id: scenario.id,
      url: scenario.url,
      claim: scenario.claim,
      expectedVerdict: scenario.expectedVerdict,
      expectedMode: scenario.expectedMode,
      rawVerdict: raw.finalVerdict,
      verifiedVerdict: verified.finalVerdict,
      rawConfidenceBand: raw.confidenceBand,
      verifiedConfidenceBand: verified.confidenceBand,
      rawReviewRecommended: raw.reviewRecommended,
      verifiedReviewRecommended: verified.reviewRecommended,
      rawSupportSnippetCount: raw.supportSnippetCount,
      verifiedSupportSnippetCount: verified.supportSnippetCount,
      rawHasVerdictExplanation: raw.hasVerdictExplanation,
      verifiedHasVerdictExplanation: verified.hasVerdictExplanation,
      rawVerificationVerdict: raw.verificationVerdict,
      verifiedVerificationVerdict: verified.verificationVerdict,
      rawReason: raw.reason,
      verifiedReason: verified.reason,
      rawVerdictExplanation: raw.verdictExplanation,
      verifiedVerdictExplanation: verified.verdictExplanation,
      rawNextActionHint: raw.nextActionHint,
      verifiedNextActionHint: verified.nextActionHint,
      improvedByVerifier: raw.finalVerdict !== verified.finalVerdict,
    };
  } catch (error) {
    return {
      id: scenario.id,
      url: scenario.url,
      claim: scenario.claim,
      expectedVerdict: scenario.expectedVerdict,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

async function runExtract(scenario, verifier) {
  const args = [
    "target/debug/touch-browser",
    "extract",
    shellEscape(scenario.url),
    "--allow-domain",
    shellEscape(scenario.allowDomain),
    "--claim",
    shellEscape(scenario.claim),
  ];

  if (verifier) {
    args.push("--verifier-command", shellEscape(verifier));
  }

  const stdout = await runShell(args.join(" "));
  const parsed = JSON.parse(stdout);
  const output = parsed.extract.output;
  const claimOutcomes = Array.isArray(output.claimOutcomes)
    ? output.claimOutcomes
    : [];
  const matchedClaim = claimOutcomes.find(
    (claim) => claim.statement === scenario.claim,
  );

  return {
    finalVerdict: matchedClaim?.verdict ?? "unknown",
    confidenceBand: matchedClaim?.confidenceBand ?? null,
    reviewRecommended: matchedClaim?.reviewRecommended ?? false,
    supportSnippetCount: Array.isArray(matchedClaim?.supportSnippets)
      ? matchedClaim.supportSnippets.length
      : 0,
    hasVerdictExplanation: Boolean(matchedClaim?.verdictExplanation),
    verdictExplanation: matchedClaim?.verdictExplanation ?? null,
    verificationVerdict: matchedClaim?.verificationVerdict ?? null,
    reason: matchedClaim?.reason ?? null,
    nextActionHint: matchedClaim?.nextActionHint ?? null,
  };
}

function isSupportedVerdict(verdict) {
  return verdict === "evidence-supported";
}

function isUnsafeOrAutoAnswerCandidate(prefix) {
  return (scenario) =>
    scenario[`${prefix}Verdict`] === "evidence-supported" &&
    scenario[`${prefix}ConfidenceBand`] === "high" &&
    scenario[`${prefix}ReviewRecommended`] === false;
}

function reviewCaptured(scenario, prefix) {
  return (
    scenario[`${prefix}ReviewRecommended`] === true ||
    scenario[`${prefix}Verdict`] === "needs-more-browsing" ||
    scenario[`${prefix}Verdict`] === "insufficient-evidence"
  );
}

function reportGateSatisfied({
  rawHighBandPredictions,
  verifiedHighBandPredictions,
  reviewRequiredScenarios,
  rawReviewCapturedCount,
  verifiedReviewCapturedCount,
  explainabilityCoveredCount,
  verifiedExplainabilityCoveredCount,
  successfulCount,
}) {
  const rawUnsafeAutoAnswerCount = rawHighBandPredictions.filter(
    (scenario) => scenario.expectedMode !== "auto-answer",
  ).length;
  const verifiedUnsafeAutoAnswerCount = verifiedHighBandPredictions.filter(
    (scenario) => scenario.expectedMode !== "auto-answer",
  ).length;
  const rawReviewCaptureRate =
    reviewRequiredScenarios.length === 0
      ? 1
      : rawReviewCapturedCount / reviewRequiredScenarios.length;
  const verifiedReviewCaptureRate =
    reviewRequiredScenarios.length === 0
      ? 1
      : verifiedReviewCapturedCount / reviewRequiredScenarios.length;
  const rawExplainabilityCoverage =
    successfulCount === 0 ? 0 : explainabilityCoveredCount / successfulCount;
  const verifiedExplainabilityCoverage =
    successfulCount === 0
      ? 0
      : verifiedExplainabilityCoveredCount / successfulCount;

  return (
    rawUnsafeAutoAnswerCount === 0 &&
    verifiedUnsafeAutoAnswerCount === 0 &&
    rawReviewCaptureRate >= 1 &&
    verifiedReviewCaptureRate >= 1 &&
    rawExplainabilityCoverage >= 1 &&
    verifiedExplainabilityCoverage >= 1
  );
}

await main();
