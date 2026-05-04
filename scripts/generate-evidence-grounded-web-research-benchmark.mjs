import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = fileURLToPath(new URL("..", import.meta.url));
const scenariosRoot = path.join(repoRoot, "fixtures/scenarios");
const outputDir = path.join(
  scenariosRoot,
  "evidence-grounded-web-research-benchmark",
);
const outputPath = path.join(outputDir, "report.json");

const publicWeb = await readReport("public-web-benchmark");
const realUser = await readReport("real-user-research-benchmark");
const adversarial = await readReport("adversarial-benchmark");
const citation = await readReport("citation-metrics");
const toolComparison = await readReport("tool-comparison-benchmark");

const seedClaimOrFixtureCount =
  numberAt(publicWeb, ["taskProof", "extractedClaimCount"]) +
  numberAt(realUser, ["totalExtractedClaimCount"]) +
  numberAt(adversarial, ["sampleCount"]) +
  numberAt(citation, ["fixtureCount"]) +
  numberAt(toolComparison, ["positiveClaimCount"]) +
  numberAt(toolComparison, ["negativeClaimCount"]);

const touchBrowserFalsePositiveRate = numberAt(toolComparison, [
  "surfaces",
  "touchBrowserExtract",
  "plausibleNegativeFalsePositiveRate",
]);
const markdownFalsePositiveRate = numberAt(toolComparison, [
  "surfaces",
  "markdownBaseline",
  "plausibleNegativeFalsePositiveRate",
]);

const qualityGates = {
  publicWebSupportedClaimRate: numberAt(publicWeb, [
    "taskProof",
    "supportedClaimRate",
  ]),
  realUserSupportedClaimRate: numberAt(realUser, ["averageSupportedClaimRate"]),
  verifiedAdversarialAccuracy: numberAt(adversarial, [
    "verifiedExactVerdictAccuracy",
  ]),
  unsafeAutoAnswerCount: numberAt(adversarial, [
    "verifiedUnsafeAutoAnswerCount",
  ]),
  citationPrecision: numberAt(citation, ["averageCitationPrecision"]),
  unsupportedPrecision: numberAt(citation, ["averageUnsupportedPrecision"]),
  supportReferencePrecision: numberAt(citation, [
    "averageSupportReferencePrecision",
  ]),
  touchBrowserFalsePositiveRate,
  markdownFalsePositiveRate,
  falsePositiveDelta: round2(
    markdownFalsePositiveRate - touchBrowserFalsePositiveRate,
  ),
};

const status =
  qualityGates.publicWebSupportedClaimRate >= 0.95 &&
  qualityGates.realUserSupportedClaimRate >= 0.95 &&
  qualityGates.verifiedAdversarialAccuracy >= 0.95 &&
  qualityGates.unsafeAutoAnswerCount === 0 &&
  qualityGates.citationPrecision >= 0.95 &&
  qualityGates.unsupportedPrecision >= 0.95 &&
  qualityGates.supportReferencePrecision >= 0.95 &&
  touchBrowserFalsePositiveRate <= markdownFalsePositiveRate
    ? "seed-validated"
    : "partial";

const report = {
  checkedAt: new Date().toISOString(),
  benchmark: "Evidence-Grounded Web Research Benchmark",
  version: "v1-seed",
  status,
  seedClaimOrFixtureCount,
  targetClaimCount: 1000,
  methodology: {
    unit: "claim or citation fixture",
    principle:
      "Measure evidence-grounded web research as claim verification with citations, abstention/review routing, and false-positive control rather than answer generation.",
    includedReports: [
      "public-web-benchmark",
      "real-user-research-benchmark",
      "adversarial-benchmark",
      "citation-metrics",
      "tool-comparison-benchmark",
    ],
  },
  qualityGates,
};

await mkdir(outputDir, { recursive: true });
await writeFile(outputPath, `${JSON.stringify(report, null, 2)}\n`);
process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);

async function readReport(name) {
  const reportPath = path.join(scenariosRoot, name, "report.json");
  return JSON.parse(await readFile(reportPath, "utf8"));
}

function numberAt(value, pathSegments) {
  let current = value;
  for (const segment of pathSegments) {
    current = current?.[segment];
  }
  if (typeof current !== "number") {
    throw new TypeError(
      `Expected numeric report field: ${pathSegments.join(".")}`,
    );
  }
  return current;
}

function round2(value) {
  return Math.round(value * 100) / 100;
}
