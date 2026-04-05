import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import {
  ensureCliBuilt,
  normalizeCleanedDom,
  normalizeText,
  repoRoot,
  roundTo,
  runShell,
  shellEscape,
  stripHtml,
} from "./lib/live-sample-server.mjs";
import {
  averageMetric,
  buildObservationTokenMetrics,
} from "./lib/observation-metrics.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";
import { createServeRpcClient } from "./lib/serve-rpc-client.mjs";

const publicSamples = [
  {
    id: "iana-reserved-domains",
    url: "https://www.iana.org/domains/reserved",
    allowDomain: "www.iana.org",
    mustContainTexts: ["IANA-managed Reserved Domains"],
  },
  {
    id: "iana-example-domains",
    url: "https://www.iana.org/domains/example",
    allowDomain: "www.iana.org",
    mustContainTexts: ["Example Domains"],
    claim:
      "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes.",
  },
  {
    id: "rfc-9309",
    url: "https://www.rfc-editor.org/rfc/rfc9309.html",
    allowDomain: "www.rfc-editor.org",
    mustContainTexts: ["Robots Exclusion Protocol"],
    claim: "RFC 9309 specifies the Robots Exclusion Protocol.",
  },
  {
    id: "rfc-2606",
    url: "https://www.rfc-editor.org/rfc/rfc2606.html",
    allowDomain: "www.rfc-editor.org",
    mustContainTexts: ["Reserved Top Level DNS Names"],
    claim: "RFC 2606 is titled Reserved Top Level DNS Names.",
  },
  {
    id: "rfc-6761",
    url: "https://www.rfc-editor.org/rfc/rfc6761.html",
    allowDomain: "www.rfc-editor.org",
    mustContainTexts: ["Special-Use Domain Names"],
    claim: "RFC 6761 is titled Special-Use Domain Names.",
  },
];

async function main() {
  await ensureCliBuilt();

  const fixtures = [];

  for (const sample of publicSamples) {
    try {
      const html = await fetchHtml(sample.url);
      const cleanedDom = normalizeCleanedDom(html);
      const visibleText = normalizeText(stripHtml(html));

      const runtimeSnapshot = await cliSnapshot(
        sample.url,
        sample.allowDomain,
        false,
      );
      const browserSnapshot = await cliSnapshot(
        sample.url,
        sample.allowDomain,
        true,
      );
      const rawHtmlTokens = countTokens(html);
      const cleanedDomTokens = countTokens(cleanedDom);
      const visibleTextTokens = countTokens(visibleText);

      fixtures.push({
        id: sample.id,
        sourceUrl: sample.url,
        allowDomain: sample.allowDomain,
        runtime: metricEntry(
          sample.mustContainTexts,
          runtimeSnapshot,
          rawHtmlTokens,
          cleanedDomTokens,
          visibleTextTokens,
        ),
        browser: metricEntry(
          sample.mustContainTexts,
          browserSnapshot,
          rawHtmlTokens,
          cleanedDomTokens,
          visibleTextTokens,
        ),
      });
    } catch (error) {
      fixtures.push({
        id: sample.id,
        sourceUrl: sample.url,
        allowDomain: sample.allowDomain,
        error: error instanceof Error ? error.message : String(error),
      });
    }
  }

  const successfulFixtures = fixtures.filter(
    (fixture) => !("error" in fixture),
  );
  const synthesis = await runServeBenchmark(
    publicSamples.filter((sample) =>
      successfulFixtures.some((fixture) => fixture.id === sample.id),
    ),
  );

  const report = {
    sampleCount: publicSamples.length,
    successfulSampleCount: successfulFixtures.length,
    averageRuntimeHtmlTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.runtime.htmlTokenizerReductionRatio,
    ),
    averageRuntimeCleanedDomTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.runtime.cleanedDomTokenizerReductionRatio,
    ),
    averageBrowserHtmlTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.browser.htmlTokenizerReductionRatio,
    ),
    averageBrowserCleanedDomTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.browser.cleanedDomTokenizerReductionRatio,
    ),
    averageRuntimeMustContainRecall: averageMetric(
      successfulFixtures,
      (fixture) => fixture.runtime.mustContainRecall,
    ),
    averageRuntimeReadingHtmlTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.runtime.readingHtmlTokenizerReductionRatio,
    ),
    averageRuntimeReadingCleanedDomTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.runtime.readingCleanedDomTokenizerReductionRatio,
    ),
    averageBrowserMustContainRecall: averageMetric(
      successfulFixtures,
      (fixture) => fixture.browser.mustContainRecall,
    ),
    averageBrowserReadingHtmlTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.browser.readingHtmlTokenizerReductionRatio,
    ),
    averageBrowserReadingCleanedDomTokenizerReductionRatio: averageMetric(
      successfulFixtures,
      (fixture) => fixture.browser.readingCleanedDomTokenizerReductionRatio,
    ),
    synthesis,
    taskProof: synthesis.taskProof,
    fixtures,
    status:
      successfulFixtures.length === publicSamples.length &&
      synthesis.status === "ok"
        ? "public-alpha"
        : "partial",
  };

  await writeRepoJson(
    "fixtures/scenarios/public-web-benchmark/report.json",
    report,
  );
}

function metricEntry(
  mustContainTexts,
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
      mustContainTexts,
    }),
  };
}

async function cliSnapshot(url, allowDomain, browser) {
  const args = ["open", url, "--allow-domain", allowDomain];
  if (browser) {
    args.push("--browser");
  }
  const stdout = await runShell(
    `target/debug/touch-browser ${args.map(shellEscape).join(" ")}`,
  );
  const parsed = JSON.parse(stdout);
  return parsed.output;
}

async function fetchHtml(url) {
  try {
    const response = await fetch(url, {
      redirect: "follow",
      headers: {
        "user-agent": "touch-browser/0.1 public benchmark",
      },
    });
    if (!response.ok) {
      throw new Error(`HTTP ${response.status} while fetching ${url}`);
    }
    return await response.text();
  } catch {
    return await runShell(`curl -LfsS ${shellEscape(url)}`);
  }
}

async function runServeBenchmark(samples) {
  if (samples.length === 0) {
    return {
      status: "skipped",
      reason: "No successful public samples were available.",
    };
  }

  const client = createServeRpcClient();
  let sessionId = null;
  try {
    const created = await client.call("runtime.session.create", {
      allowDomains: [...new Set(samples.map((sample) => sample.allowDomain))],
    });
    sessionId = created.sessionId;

    const openedTabs = [];
    const extracts = [];
    for (let index = 0; index < samples.length; index += 1) {
      const sample = samples[index];
      const result =
        index === 0
          ? await client.call("runtime.session.open", {
              sessionId: created.sessionId,
              target: sample.url,
            })
          : await client.call("runtime.tab.open", {
              sessionId: created.sessionId,
              target: sample.url,
            });
      openedTabs.push({
        sampleId: sample.id,
        tabId: result.tabId,
        openStatus: result.result.status,
      });

      await client.call("runtime.tab.select", {
        sessionId: created.sessionId,
        tabId: result.tabId,
      });

      if (sample.claim) {
        const extract = await client.call("runtime.session.extract", {
          sessionId: created.sessionId,
          claims: [sample.claim],
        });
        extracts.push(summarizeTaskExtract(sample, result.tabId, extract));
      }
    }

    const synthesis = await client.call("runtime.session.synthesize", {
      sessionId,
      noteLimit: 10,
    });
    await client.call("runtime.session.close", {
      sessionId,
    });
    sessionId = null;

    return {
      status: "ok",
      sessionId: created.sessionId,
      tabCount: synthesis.tabCount,
      openedTabs,
      report: synthesis.report,
      tabReports: synthesis.tabReports,
      taskProof: summarizeTaskProof(extracts, synthesis),
    };
  } finally {
    await closeSessionQuietly(client, sessionId);
    await client.close();
  }
}

function summarizeTaskExtract(sample, tabId, extractResult) {
  const output = extractResult?.result?.extract?.output ?? {};
  const evidenceSupportedClaims = Array.isArray(output.evidenceSupportedClaims)
    ? output.evidenceSupportedClaims
    : [];
  const insufficientEvidenceClaims = Array.isArray(
    output.insufficientEvidenceClaims,
  )
    ? output.insufficientEvidenceClaims
    : [];
  const supported = evidenceSupportedClaims.some(
    (claim) => claim.statement === sample.claim,
  );
  const unsupported = insufficientEvidenceClaims.some(
    (claim) => claim.statement === sample.claim,
  );
  const matchedSupportedClaim = evidenceSupportedClaims.find(
    (claim) => claim.statement === sample.claim,
  );

  return {
    sampleId: sample.id,
    tabId,
    claim: sample.claim,
    status: supported ? "supported" : unsupported ? "unsupported" : "unknown",
    citationCount: matchedSupportedClaim?.citations
      ? matchedSupportedClaim.citations.length
      : matchedSupportedClaim?.citation
        ? 1
        : 0,
    supportRefCount: Array.isArray(matchedSupportedClaim?.supportRefs)
      ? matchedSupportedClaim.supportRefs.length
      : Array.isArray(matchedSupportedClaim?.support)
        ? matchedSupportedClaim.support.length
        : 0,
  };
}

function summarizeTaskProof(extracts, synthesis) {
  const supportedClaimCount = extracts.filter(
    (extract) => extract.status === "supported",
  ).length;
  const unsupportedClaimCount = extracts.filter(
    (extract) => extract.status === "unsupported",
  ).length;

  return {
    question:
      "Which public sources explicitly support the documentation-domain and robots-exclusion claims relevant to research browsing?",
    extractedClaimCount: extracts.length,
    supportedClaimCount,
    unsupportedClaimCount,
    supportedClaimRate: roundTo(
      supportedClaimCount / Math.max(extracts.length, 1),
      2,
    ),
    extractedSamples: extracts,
    synthesizedNoteCount: Array.isArray(synthesis?.report?.synthesizedNotes)
      ? synthesis.report.synthesizedNotes.length
      : 0,
  };
}

async function closeSessionQuietly(client, sessionId) {
  if (!sessionId) {
    return;
  }

  try {
    await client.call("runtime.session.close", { sessionId });
  } catch {
    // Best-effort cleanup to avoid leaked daemon sessions after partial benchmark failures.
  }
}

await main();
