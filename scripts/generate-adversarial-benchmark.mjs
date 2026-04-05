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
  },
  {
    id: "lambda-limits-supported",
    url: "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "The maximum timeout for a Lambda function is 15 minutes.",
    expectedVerdict: "evidence-supported",
  },
  {
    id: "lambda-limits-contradicted",
    url: "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "The maximum timeout for a Lambda function is 24 hours.",
    expectedVerdict: "contradicted",
  },
  {
    id: "ecs-overview-needs-more-browsing",
    url: "https://docs.aws.amazon.com/AmazonECS/latest/developerguide/Welcome.html",
    allowDomain: "docs.aws.amazon.com",
    claim: "ECS supports GPU instances natively.",
    expectedVerdict: "needs-more-browsing",
  },
  {
    id: "iana-registration-contradicted",
    url: "https://www.iana.org/help/example-domains",
    allowDomain: "www.iana.org",
    claim: "Example domains are available for registration.",
    expectedVerdict: "contradicted",
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
    verifierCommand,
    scenarios: scenarioReports,
    status:
      successful.length === scenarios.length &&
      verifiedCorrectCount === successful.length
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
      rawVerdict: raw.finalVerdict,
      verifiedVerdict: verified.finalVerdict,
      rawVerificationVerdict: raw.verificationVerdict,
      verifiedVerificationVerdict: verified.verificationVerdict,
      rawReason: raw.reason,
      verifiedReason: verified.reason,
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
    verificationVerdict: matchedClaim?.verificationVerdict ?? null,
    reason: matchedClaim?.reason ?? null,
    nextActionHint: matchedClaim?.nextActionHint ?? null,
  };
}

await main();
