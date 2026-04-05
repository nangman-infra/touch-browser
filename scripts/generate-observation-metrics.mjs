import { readFile } from "node:fs/promises";
import path from "node:path";
import { countTokens } from "gpt-tokenizer/model/gpt-4o";
import {
  normalizeCleanedDom,
  normalizeText,
  repoRoot,
  roundTo,
  stripHtml,
} from "./lib/live-sample-server.mjs";
import {
  averageMetric,
  buildObservationTokenMetrics,
  estimateTokens,
  medianMetric,
  minMetric,
} from "./lib/observation-metrics.mjs";
import { listRepoFilesNamed, writeRepoJson } from "./lib/scenario-files.mjs";

async function main() {
  const metadataPaths = await listRepoFilesNamed(
    "fixtures/research",
    "fixture.json",
  );
  const fixtures = [];

  for (const metadataPath of metadataPaths) {
    fixtures.push(JSON.parse(await readFile(metadataPath, "utf8")));
  }

  const reports = [];
  for (const fixture of fixtures) {
    const html = await readFile(path.join(repoRoot, fixture.htmlPath), "utf8");
    const snapshot = JSON.parse(
      await readFile(path.join(repoRoot, fixture.expectedSnapshotPath), "utf8"),
    );
    const cleanedDom = normalizeCleanedDom(html);
    const rawHtmlTokenEstimate = estimateTokens(html);
    const cleanedDomTokenEstimate = estimateTokens(cleanedDom);
    const rawVisibleText = normalizeText(stripHtml(html));
    const rawVisibleTextTokenEstimate = estimateTokens(rawVisibleText);
    const tokenMetrics = buildObservationTokenMetrics({
      snapshot,
      rawHtmlTokens: countTokens(html),
      cleanedDomTokens: countTokens(cleanedDom),
      visibleTextTokens: countTokens(rawVisibleText),
      mustContainTexts: fixture.expectations.mustContainTexts,
    });
    const snapshotTokenEstimate = snapshot.budget.emittedTokens;

    reports.push({
      id: fixture.id,
      category: fixture.category,
      rawHtmlTokenEstimate,
      cleanedDomTokenEstimate,
      rawVisibleTextTokenEstimate,
      snapshotTokenEstimate,
      htmlReductionRatio: roundTo(
        rawHtmlTokenEstimate / Math.max(snapshotTokenEstimate, 1),
        2,
      ),
      cleanedDomReductionRatio: roundTo(
        cleanedDomTokenEstimate / Math.max(snapshotTokenEstimate, 1),
        2,
      ),
      reductionRatio: roundTo(
        rawVisibleTextTokenEstimate / Math.max(snapshotTokenEstimate, 1),
        2,
      ),
      ...tokenMetrics,
      truncated: snapshot.budget.truncated,
    });
  }

  const report = {
    fixtureCount: reports.length,
    averageHtmlReductionRatio: averageMetric(reports, "htmlReductionRatio"),
    averageCleanedDomReductionRatio: averageMetric(
      reports,
      "cleanedDomReductionRatio",
    ),
    medianHtmlReductionRatio: medianMetric(reports, "htmlReductionRatio"),
    medianCleanedDomReductionRatio: medianMetric(
      reports,
      "cleanedDomReductionRatio",
    ),
    minHtmlReductionRatio: minMetric(reports, "htmlReductionRatio"),
    minCleanedDomReductionRatio: minMetric(reports, "cleanedDomReductionRatio"),
    averageVisibleTextRatio: averageMetric(reports, "reductionRatio"),
    averageHtmlTokenizerReductionRatio: averageMetric(
      reports,
      "htmlTokenizerReductionRatio",
    ),
    averageCleanedDomTokenizerReductionRatio: averageMetric(
      reports,
      "cleanedDomTokenizerReductionRatio",
    ),
    averageVisibleTextTokenizerReductionRatio: averageMetric(
      reports,
      "visibleTextTokenizerReductionRatio",
    ),
    averageReadingHtmlTokenizerReductionRatio: averageMetric(
      reports,
      "readingHtmlTokenizerReductionRatio",
    ),
    averageReadingCleanedDomTokenizerReductionRatio: averageMetric(
      reports,
      "readingCleanedDomTokenizerReductionRatio",
    ),
    averageReadingVisibleTextTokenizerReductionRatio: averageMetric(
      reports,
      "readingVisibleTextTokenizerReductionRatio",
    ),
    medianHtmlTokenizerReductionRatio: medianMetric(
      reports,
      "htmlTokenizerReductionRatio",
    ),
    medianCleanedDomTokenizerReductionRatio: medianMetric(
      reports,
      "cleanedDomTokenizerReductionRatio",
    ),
    minHtmlTokenizerReductionRatio: minMetric(
      reports,
      "htmlTokenizerReductionRatio",
    ),
    minCleanedDomTokenizerReductionRatio: minMetric(
      reports,
      "cleanedDomTokenizerReductionRatio",
    ),
    averageMustContainRecall: averageMetric(reports, "mustContainRecall"),
    fixtures: reports,
  };

  await writeRepoJson(
    "fixtures/scenarios/observation-metrics/report.json",
    report,
  );
}

await main();
