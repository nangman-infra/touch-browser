import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import {
  ensureCliBuilt,
  liveSamples,
  normalizeCleanedDom,
  normalizeText,
  runShell,
  shellEscape,
  stripHtml,
  withLiveSampleServer,
} from "./lib/live-sample-server.mjs";
import {
  averageMetric,
  buildObservationTokenMetrics,
} from "./lib/observation-metrics.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

async function main() {
  await ensureCliBuilt();

  const report = await withLiveSampleServer(async ({ baseUrl }) => {
    const fixtures = [];

    for (const sample of liveSamples) {
      const url = `${baseUrl}${sample.path}`;
      const html = sample.html;
      const cleanedDom = normalizeCleanedDom(html);
      const visibleText = normalizeText(stripHtml(html));

      const runtimeSnapshot = await cliSnapshot("open", [url]);
      const browserSnapshot = await cliSnapshot("open", [url, "--browser"]);
      const rawHtmlTokens = countTokens(html);
      const cleanedDomTokens = countTokens(cleanedDom);
      const visibleTextTokens = countTokens(visibleText);

      fixtures.push({
        id: sample.id,
        sourceUrl: url,
        runtime: metricEntry(
          sample,
          runtimeSnapshot,
          rawHtmlTokens,
          cleanedDomTokens,
          visibleTextTokens,
        ),
        browser: metricEntry(
          sample,
          browserSnapshot,
          rawHtmlTokens,
          cleanedDomTokens,
          visibleTextTokens,
        ),
      });
    }

    return {
      fixtureCount: fixtures.length,
      averageRuntimeHtmlTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.runtime.htmlTokenizerReductionRatio,
      ),
      averageRuntimeCleanedDomTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.runtime.cleanedDomTokenizerReductionRatio,
      ),
      averageBrowserHtmlTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.browser.htmlTokenizerReductionRatio,
      ),
      averageBrowserCleanedDomTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.browser.cleanedDomTokenizerReductionRatio,
      ),
      averageRuntimeMustContainRecall: averageMetric(
        fixtures,
        (fixture) => fixture.runtime.mustContainRecall,
      ),
      averageRuntimeReadingHtmlTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.runtime.readingHtmlTokenizerReductionRatio,
      ),
      averageRuntimeReadingCleanedDomTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.runtime.readingCleanedDomTokenizerReductionRatio,
      ),
      averageBrowserMustContainRecall: averageMetric(
        fixtures,
        (fixture) => fixture.browser.mustContainRecall,
      ),
      averageBrowserReadingHtmlTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.browser.readingHtmlTokenizerReductionRatio,
      ),
      averageBrowserReadingCleanedDomTokenizerReductionRatio: averageMetric(
        fixtures,
        (fixture) => fixture.browser.readingCleanedDomTokenizerReductionRatio,
      ),
      fixtures,
    };
  });

  await writeRepoJson(
    "fixtures/scenarios/live-observation-metrics/report.json",
    report,
  );
}

function metricEntry(
  sample,
  snapshot,
  rawHtmlTokens,
  cleanedDomTokens,
  visibleTextTokens,
) {
  return {
    ...buildObservationTokenMetrics({
      snapshot,
      rawHtmlTokens,
      cleanedDomTokens,
      visibleTextTokens,
      mustContainTexts: sample.mustContainTexts,
    }),
  };
}

async function cliSnapshot(command, args) {
  const stdout = await runShell(
    `target/debug/touch-browser ${command} ${args
      .map(shellEscape)
      .join(" ")} --allow-domain 127.0.0.1`,
  );
  const parsed = JSON.parse(stdout);
  return parsed.output;
}

await main();
