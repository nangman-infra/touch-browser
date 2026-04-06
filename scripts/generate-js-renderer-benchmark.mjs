import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import { claimOutcomeForStatement } from "./lib/evidence-report.mjs";
import {
  ensureCliBuilt,
  normalizeText,
  roundTo,
  runShell,
  shellEscape,
} from "./lib/live-sample-server.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath = "fixtures/scenarios/js-renderer-benchmark/report.json";

const samples = [
  {
    id: "firecrawl-docs-introduction",
    url: "https://docs.firecrawl.dev/introduction",
    pageType: "docs",
    mustContainTexts: ["Firecrawl", "Search", "Scrape", "Interact"],
    claim:
      "Firecrawl offers Search, Scrape, and Interact as core capabilities.",
  },
  {
    id: "react-router-home",
    url: "https://reactrouter.com/home",
    pageType: "docs",
    mustContainTexts: ["React Router", "Framework", "Declarative"],
    claim:
      "React Router supports both declarative routing and framework-style features for modern React apps.",
  },
  {
    id: "vercel-docs",
    url: "https://vercel.com/docs",
    pageType: "docs",
    mustContainTexts: ["Vercel", "AI Cloud"],
    claim: "Vercel is the AI Cloud.",
  },
  {
    id: "firecrawl-playground",
    url: "https://firecrawl.dev/playground",
    pageType: "app",
    expectedSourceType: "playwright",
    mustContainTexts: ["Playground"],
  },
];

async function main() {
  await ensureCliBuilt();

  const sampleReports = [];
  for (const sample of samples) {
    sampleReports.push(await evaluateSample(sample));
  }

  const successfulSamples = sampleReports.filter((sample) => !sample.error);
  const docSamples = successfulSamples.filter(
    (sample) => sample.pageType === "docs",
  );
  const appSamples = successfulSamples.filter(
    (sample) => sample.pageType === "app",
  );

  const docsSupportedCount = docSamples.filter(
    (sample) => sample.extract?.passed,
  ).length;
  const mustContainRecallRate =
    successfulSamples.length === 0
      ? 0
      : roundTo(
          successfulSamples.reduce(
            (sum, sample) => sum + sample.mustContainRecall,
            0,
          ) / successfulSamples.length,
          2,
        );
  const appAutoPlaywrightCount = appSamples.filter(
    (sample) =>
      sample.open.sourceType === sample.expectedSourceType &&
      sample.mainOnly.passed,
  ).length;

  const report = {
    checkedAt: new Date().toISOString(),
    status:
      successfulSamples.length === samples.length &&
      docSamples.length > 0 &&
      docsSupportedCount === docSamples.length &&
      mustContainRecallRate >= 0.9 &&
      appAutoPlaywrightCount === appSamples.length
        ? "js-renderer-validated"
        : "partial",
    sampleCount: samples.length,
    successfulSampleCount: successfulSamples.length,
    docsSampleCount: docSamples.length,
    docsSupportedCount,
    docsSupportedRate:
      docSamples.length === 0
        ? 0
        : roundTo(docsSupportedCount / docSamples.length, 2),
    mustContainRecallRate,
    appAutoPlaywrightCount,
    appAutoPlaywrightRate:
      appSamples.length === 0
        ? 0
        : roundTo(appAutoPlaywrightCount / appSamples.length, 2),
    samples: sampleReports,
  };

  await writeRepoJson(reportPath, report);
}

async function evaluateSample(sample) {
  try {
    const openResult = await openAuto(sample.url);
    const mainOnlyContent = await readMainOnlyView(sample.url);
    const matchedMustContain = sample.mustContainTexts.filter((expectedText) =>
      normalizedContains(mainOnlyContent, expectedText),
    );

    const report = {
      id: sample.id,
      pageType: sample.pageType,
      url: sample.url,
      expectedSourceType: sample.expectedSourceType ?? null,
      open: {
        sourceType: openResult.source.sourceType,
        title: openResult.source.title ?? null,
        emittedTokens: openResult.budget.emittedTokens,
        blockCount: openResult.blocks.length,
      },
      mainOnly: {
        tokens: countTokens(mainOnlyContent),
        matchedMustContain,
        passed:
          matchedMustContain.length === sample.mustContainTexts.length &&
          mainOnlyContent.trim().length > 0 &&
          (sample.pageType !== "app" || countTokens(mainOnlyContent) <= 32),
      },
      mustContainRecall:
        sample.mustContainTexts.length === 0
          ? 1
          : roundTo(
              matchedMustContain.length / sample.mustContainTexts.length,
              2,
            ),
    };

    if (!sample.claim) {
      return report;
    }

    const extractResult = await extractClaim(sample.url, sample.claim);
    const outcome = claimOutcomeForStatement(extractResult, sample.claim);

    return {
      ...report,
      extract: {
        verdict: outcome?.verdict ?? null,
        supportScore: Number(outcome?.supportScore ?? 0),
        checkedBlockRefs: outcome?.checkedBlockRefs ?? [],
        passed: outcome?.verdict === "evidence-supported",
      },
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

async function openAuto(url) {
  const stdout = await runShell(
    `target/debug/touch-browser open ${shellEscape(url)}`,
  );
  return JSON.parse(stdout).output;
}

async function readMainOnlyView(url) {
  return await runShell(
    `target/debug/touch-browser read-view ${shellEscape(url)} --main-only`,
  );
}

async function extractClaim(url, claim) {
  const stdout = await runShell(
    `target/debug/touch-browser extract ${shellEscape(url)} --claim ${shellEscape(claim)}`,
  );
  return JSON.parse(stdout).extract.output;
}

function normalizedContains(content, expected) {
  const normalizedContent = normalizeText(content).toLowerCase();
  const normalizedExpected = normalizeText(expected).toLowerCase();
  return normalizedContent.includes(normalizedExpected);
}

await main();
