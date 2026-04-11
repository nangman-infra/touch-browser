import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import {
  ensureCliBuilt,
  normalizeText,
  roundTo,
  runShell,
  shellEscape,
} from "./lib/live-sample-server.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath = "fixtures/scenarios/aws-page-type-benchmark/report.json";

const samples = [
  {
    id: "lambda-quotas",
    pageType: "quota-guide",
    url: "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html",
    mustContainTexts: ["Lambda quotas", "15 minutes", "10,240 MB of memory"],
  },
  {
    id: "s3-listobjectsv2-api",
    pageType: "api-reference",
    url: "https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectsV2.html",
    mustContainTexts: ["ListObjectsV2", "up to 1,000", "General purpose bucket"],
  },
  {
    id: "sdk-js-s3-examples",
    pageType: "code-examples",
    url: "https://docs.aws.amazon.com/sdk-for-javascript/v3/developer-guide/javascript_s3_code_examples.html",
    mustContainTexts: [
      "Amazon S3 examples using SDK for JavaScript (v3)",
      "AWS SDK for JavaScript (v3)",
      "AWS Code Examples Repository",
    ],
  },
  {
    id: "prescriptive-guidance-patterns",
    pageType: "prescriptive-guidance",
    url: "https://docs.aws.amazon.com/prescriptive-guidance/latest/patterns/migrate-databases-to-amazon-rds.html",
    mustContainTexts: [
      "AWS Prescriptive Guidance Patterns",
      "step-by-step instructions",
      "migrating to AWS",
    ],
  },
  {
    id: "s3-files-guide",
    pageType: "storage-guide",
    url: "https://docs.aws.amazon.com/AmazonS3/latest/userguide/s3-files.html",
    mustContainTexts: [
      "Working with Amazon S3 Files",
      "shared file system",
      "high-performance storage",
    ],
  },
  {
    id: "cloudformation-template-reference",
    pageType: "template-reference",
    url: "https://docs.aws.amazon.com/AWSCloudFormation/latest/TemplateReference/aws-properties-s3vectors-index-metadatafileconfiguration.html",
    mustContainTexts: [
      "CloudFormation Template Reference Guide",
      "AWS resource and property types reference",
      "Intrinsic functions",
    ],
  },
];

async function main() {
  await ensureCliBuilt();
  await runShell("pnpm run build:playwright-runtime");

  const sampleReports = [];
  for (const sample of samples) {
    sampleReports.push(await evaluateSample(sample));
  }

  const successfulSamples = sampleReports.filter((sample) => !sample.error);
  const mainOnlyRecallRate =
    successfulSamples.length === 0
      ? 0
      : roundTo(
          successfulSamples.reduce(
            (sum, sample) => sum + sample.mainOnly.recall,
            0,
          ) / successfulSamples.length,
          2,
        );
  const browserRicherCount = successfulSamples.filter(
    (sample) => sample.browserRicher,
  ).length;
  const autoFallbackCount = successfulSamples.filter(
    (sample) => sample.auto.diagnostics.captureMode === "browser-fallback",
  ).length;

  const report = {
    checkedAt: new Date().toISOString(),
    status:
      successfulSamples.length === samples.length
        ? "aws-page-types-profiled"
        : "partial",
    sampleCount: samples.length,
    successfulSampleCount: successfulSamples.length,
    mainOnlyRecallRate,
    browserRicherCount,
    browserRicherRate:
      successfulSamples.length === 0
        ? 0
        : roundTo(browserRicherCount / successfulSamples.length, 2),
    autoFallbackCount,
    autoFallbackRate:
      successfulSamples.length === 0
        ? 0
        : roundTo(autoFallbackCount / successfulSamples.length, 2),
    samples: sampleReports,
  };

  await writeRepoJson(reportPath, report);
}

async function evaluateSample(sample) {
  try {
    const auto = await openSnapshot(sample.url, false);
    const browser = await openSnapshot(sample.url, true);
    const mainOnly = await readMainOnly(sample.url, sample.mustContainTexts);

    return {
      id: sample.id,
      pageType: sample.pageType,
      url: sample.url,
      auto,
      browser,
      browserRicher:
        browser.emittedTokens > auto.emittedTokens ||
        browser.diagnostics.mainBlockCount > auto.diagnostics.mainBlockCount ||
        browser.blockCount > auto.blockCount,
      mainOnly,
    };
  } catch (error) {
    return {
      id: sample.id,
      pageType: sample.pageType,
      url: sample.url,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

async function openSnapshot(url, forcedBrowser) {
  const command = forcedBrowser
    ? `target/debug/touch-browser open ${shellEscape(url)} --browser`
    : `target/debug/touch-browser open ${shellEscape(url)}`;
  const startedAt = Date.now();
  const stdout = await runShell(command);
  const latencyMs = Date.now() - startedAt;
  const result = JSON.parse(stdout);

  return {
    latencyMs,
    sourceType: result.output.source.sourceType,
    title: result.output.source.title ?? null,
    emittedTokens: result.output.budget.emittedTokens,
    blockCount: result.output.blocks.length,
    diagnostics: result.diagnostics,
  };
}

async function readMainOnly(url, mustContainTexts) {
  const command = `target/debug/touch-browser read-view ${shellEscape(url)} --main-only`;
  const startedAt = Date.now();
  const content = await runShell(command);
  const latencyMs = Date.now() - startedAt;
  const matchedMustContain = mustContainTexts.filter((expectedText) =>
    normalizedContains(content, expectedText),
  );

  return {
    latencyMs,
    tokens: countTokens(content),
    matchedMustContain,
    recall:
      mustContainTexts.length === 0
        ? 1
        : roundTo(matchedMustContain.length / mustContainTexts.length, 2),
    passed:
      content.trim().length > 0 &&
      matchedMustContain.length === mustContainTexts.length,
  };
}

function normalizedContains(content, expected) {
  const normalizedContent = normalizeText(content).toLowerCase();
  const normalizedExpected = normalizeText(expected).toLowerCase();
  return normalizedContent.includes(normalizedExpected);
}

await main();
