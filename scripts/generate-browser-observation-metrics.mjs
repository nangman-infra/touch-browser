import { readFile } from "node:fs/promises";
import path from "node:path";
import { countTokens } from "gpt-tokenizer/model/gpt-4o";
import {
  ensureCliBuilt,
  normalizeCleanedDom,
  normalizeText,
  repoRoot,
  runShell,
  shellEscape,
  stripHtml,
} from "./lib/live-sample-server.mjs";
import {
  averageMetric,
  buildObservationTokenMetrics,
} from "./lib/observation-metrics.mjs";
import { readRepoJson, writeRepoJson } from "./lib/scenario-files.mjs";

const fixtureTargets = [
  "fixture://research/static-docs/getting-started",
  "fixture://research/static-docs/security-model",
  "fixture://research/citation-heavy/pricing",
  "fixture://research/citation-heavy/release-notes",
  "fixture://research/navigation/browser-follow",
  "fixture://research/navigation/browser-pagination",
  "fixture://research/hostile/fake-system-message",
  "fixture://research/hostile/credential-warning",
];

async function main() {
  await ensureCliBuilt();

  const reports = [];
  for (const fixtureTarget of fixtureTargets) {
    const fixture = await readFixture(fixtureTarget);
    const snapshot = await renderBrowserSnapshot(fixtureTarget);
    const html = await readFile(path.join(repoRoot, fixture.htmlPath), "utf8");
    const cleanedDom = normalizeCleanedDom(html);
    const rawVisibleText = normalizeText(stripHtml(html));
    const tokenMetrics = buildObservationTokenMetrics({
      snapshot,
      rawHtmlTokens: countTokens(html),
      cleanedDomTokens: countTokens(cleanedDom),
      visibleTextTokens: countTokens(rawVisibleText),
      mustContainTexts: fixture.expectations.mustContainTexts,
    });

    reports.push({
      id: fixture.id,
      category: fixture.category,
      sourceType: snapshot.source.sourceType,
      ...tokenMetrics,
      truncated: snapshot.budget.truncated,
    });
  }

  const report = {
    fixtureCount: reports.length,
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
    averageMustContainRecall: averageMetric(reports, "mustContainRecall"),
    fixtures: reports,
  };

  await writeRepoJson(
    "fixtures/scenarios/browser-observation-metrics/report.json",
    report,
  );
}

async function readFixture(target) {
  const parts = target.replace("fixture://research/", "").split("/");
  const [category, id] = parts;
  const metadataPath = path.join(
    repoRoot,
    "fixtures",
    "research",
    category,
    id,
    "fixture.json",
  );
  return await readRepoJson(path.relative(repoRoot, metadataPath));
}

async function renderBrowserSnapshot(target) {
  const stdout = await runShell(
    `target/debug/touch-browser open ${shellEscape(target)} --browser`,
  );
  const parsed = JSON.parse(stdout);
  return parsed.output;
}

await main();
