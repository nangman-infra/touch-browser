import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import {
  ensureCliBuilt,
  normalizeText,
  roundTo,
  runShell,
  shellEscape,
  stripHtml,
} from "./lib/live-sample-server.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath = "fixtures/scenarios/tool-comparison-benchmark/report.json";

const samples = [
  {
    id: "aws-ecs-welcome",
    url: "https://docs.aws.amazon.com/AmazonECS/latest/developerguide/Welcome.html",
    allowDomain: "docs.aws.amazon.com",
    positiveClaims: [
      "Amazon ECS is a fully managed container orchestration service.",
    ],
    negativeClaims: [
      "ECS supports GPU instances natively.",
      "ECS is available in all AWS regions.",
    ],
  },
  {
    id: "iana-example-domains",
    url: "https://www.iana.org/help/example-domains",
    allowDomain: "www.iana.org",
    positiveClaims: [
      "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes.",
    ],
    negativeClaims: [
      "Example domains are available for registration.",
      "Example domains require prior coordination with IANA before use in documents.",
    ],
  },
  {
    id: "mdn-fetch-api",
    url: "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API",
    allowDomain: "developer.mozilla.org",
    positiveClaims: [
      "The Fetch API provides an interface for fetching resources.",
    ],
    negativeClaims: ["The Fetch API only works with XMLHttpRequest."],
  },
  {
    id: "node-path",
    url: "https://nodejs.org/api/path.html",
    allowDomain: "nodejs.org",
    positiveClaims: [
      "The node:path module provides utilities for working with file and directory paths.",
    ],
    negativeClaims: ["The node:path module performs HTTP requests."],
  },
];

async function main() {
  await ensureCliBuilt();

  const sampleReports = [];
  for (const sample of samples) {
    sampleReports.push(await evaluateSample(sample));
  }

  const successful = sampleReports.filter((sample) => !sample.error);
  const aggregate = buildAggregate(successful);
  const report = {
    checkedAt: new Date().toISOString(),
    baseline:
      "web-fetch-style markdown baseline derived from live official pages using main-content extraction and conservative lexical claim matching",
    methodology: {
      positiveClaims:
        "Official-source claims expected to be directly supported by the source page.",
      negativeClaims:
        "Plausible but unsupported claims used to measure false-positive behavior.",
      notes:
        "This is a reproducible local comparison, not a vendor API benchmark. It compares touch-browser surfaces against a markdown-first baseline tool shape.",
    },
    sampleCount: samples.length,
    successfulSampleCount: successful.length,
    positiveClaimCount: successful.reduce(
      (sum, sample) => sum + sample.positiveClaimCount,
      0,
    ),
    negativeClaimCount: successful.reduce(
      (sum, sample) => sum + sample.negativeClaimCount,
      0,
    ),
    surfaces: aggregate,
    samples: sampleReports,
    status:
      successful.length === samples.length &&
      aggregate.touchBrowserExtract.positiveClaimSupportRate >= 0.9 &&
      aggregate.touchBrowserExtract.plausibleNegativeFalsePositiveRate <=
        aggregate.markdownBaseline.plausibleNegativeFalsePositiveRate &&
      aggregate.touchBrowserCompact.averageTokens <
        aggregate.markdownBaseline.averageTokens
        ? "competitive-validated"
        : "partial",
  };

  await writeRepoJson(reportPath, report);
}

async function evaluateSample(sample) {
  try {
    const html = await fetchHtml(sample.url);
    const baselineMarkdown = htmlToMarkdownBaseline(html, sample.url);
    const readView = await readTouchBrowserReadView(sample);
    const compact = await readTouchBrowserCompactView(sample);
    const extract = await readTouchBrowserExtract(sample);

    const allClaims = [
      ...sample.positiveClaims.map((statement) => ({
        statement,
        expected: "positive",
      })),
      ...sample.negativeClaims.map((statement) => ({
        statement,
        expected: "negative",
      })),
    ];

    return {
      id: sample.id,
      url: sample.url,
      allowDomain: sample.allowDomain,
      positiveClaimCount: sample.positiveClaims.length,
      negativeClaimCount: sample.negativeClaims.length,
      tokens: {
        markdownBaseline: countTokens(baselineMarkdown),
        touchBrowserReadView: countTokens(readView.markdownText),
        touchBrowserCompact: compact.approxTokens,
      },
      markdownBaseline: summarizeTextSurface(baselineMarkdown, allClaims),
      touchBrowserReadView: summarizeTextSurface(
        readView.markdownText,
        allClaims,
      ),
      touchBrowserCompact: summarizeTextSurface(compact.compactText, allClaims),
      touchBrowserExtract: summarizeExtractSurface(extract, allClaims),
    };
  } catch (error) {
    return {
      id: sample.id,
      url: sample.url,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

function buildAggregate(successfulSamples) {
  return {
    markdownBaseline: aggregateTextSurface(
      successfulSamples,
      "markdownBaseline",
    ),
    touchBrowserReadView: aggregateTextSurface(
      successfulSamples,
      "touchBrowserReadView",
    ),
    touchBrowserCompact: aggregateTextSurface(
      successfulSamples,
      "touchBrowserCompact",
    ),
    touchBrowserExtract: aggregateExtractSurface(successfulSamples),
  };
}

function aggregateTextSurface(samples, key) {
  const positiveTotal = samples.reduce(
    (sum, sample) => sum + sample[key].positiveSupportedCount,
    0,
  );
  const positiveCount = samples.reduce(
    (sum, sample) => sum + sample.positiveClaimCount,
    0,
  );
  const negativeFalsePositives = samples.reduce(
    (sum, sample) => sum + sample[key].negativeFalsePositiveCount,
    0,
  );
  const negativeCount = samples.reduce(
    (sum, sample) => sum + sample.negativeClaimCount,
    0,
  );
  const averageTokens =
    samples.length === 0
      ? 0
      : roundTo(
          samples.reduce((sum, sample) => sum + sample.tokens[key], 0) /
            samples.length,
          2,
        );

  return {
    averageTokens,
    positiveClaimSupportRate:
      positiveCount === 0 ? 0 : roundTo(positiveTotal / positiveCount, 2),
    plausibleNegativeFalsePositiveRate:
      negativeCount === 0
        ? 0
        : roundTo(negativeFalsePositives / negativeCount, 2),
    structuredCitationCoverageRate: 0,
    stableRefCoverageRate: 0,
  };
}

function aggregateExtractSurface(samples) {
  const positiveTotal = samples.reduce(
    (sum, sample) => sum + sample.touchBrowserExtract.positiveSupportedCount,
    0,
  );
  const positiveCount = samples.reduce(
    (sum, sample) => sum + sample.positiveClaimCount,
    0,
  );
  const negativeFalsePositives = samples.reduce(
    (sum, sample) =>
      sum + sample.touchBrowserExtract.negativeFalsePositiveCount,
    0,
  );
  const negativeCount = samples.reduce(
    (sum, sample) => sum + sample.negativeClaimCount,
    0,
  );
  const supportedClaims = samples.flatMap(
    (sample) => sample.touchBrowserExtract.supportedClaims,
  );

  return {
    positiveClaimSupportRate:
      positiveCount === 0 ? 0 : roundTo(positiveTotal / positiveCount, 2),
    plausibleNegativeFalsePositiveRate:
      negativeCount === 0
        ? 0
        : roundTo(negativeFalsePositives / negativeCount, 2),
    structuredCitationCoverageRate:
      supportedClaims.length === 0
        ? 0
        : roundTo(
            supportedClaims.filter((claim) => claim.hasCitation).length /
              supportedClaims.length,
            2,
          ),
    stableRefCoverageRate:
      supportedClaims.length === 0
        ? 0
        : roundTo(
            supportedClaims.filter((claim) => claim.supportRefCount > 0)
              .length / supportedClaims.length,
            2,
          ),
    averageSupportScore:
      supportedClaims.length === 0
        ? 0
        : roundTo(
            supportedClaims.reduce(
              (sum, claim) => sum + claim.supportScore,
              0,
            ) / supportedClaims.length,
            2,
          ),
  };
}

function summarizeTextSurface(text, claims) {
  let positiveSupportedCount = 0;
  let negativeFalsePositiveCount = 0;
  const claimResults = [];

  for (const claim of claims) {
    const evaluation = evaluateClaimAgainstText(claim.statement, text);
    if (claim.expected === "positive" && evaluation.supported) {
      positiveSupportedCount += 1;
    }
    if (claim.expected === "negative" && evaluation.supported) {
      negativeFalsePositiveCount += 1;
    }
    claimResults.push({
      statement: claim.statement,
      expected: claim.expected,
      supported: evaluation.supported,
      anchorCoverage: evaluation.anchorCoverage,
      qualifierCoverage: evaluation.qualifierCoverage,
    });
  }

  return {
    positiveSupportedCount,
    negativeFalsePositiveCount,
    claimResults,
  };
}

function summarizeExtractSurface(extract, claims) {
  const supportedByStatement = new Map(
    (extract.evidenceSupportedClaims ?? []).map((claim) => [
      claim.statement,
      claim,
    ]),
  );
  let positiveSupportedCount = 0;
  let negativeFalsePositiveCount = 0;

  for (const claim of claims) {
    const supported = supportedByStatement.has(claim.statement);
    if (claim.expected === "positive" && supported) {
      positiveSupportedCount += 1;
    }
    if (claim.expected === "negative" && supported) {
      negativeFalsePositiveCount += 1;
    }
  }

  return {
    positiveSupportedCount,
    negativeFalsePositiveCount,
    supportedClaims: (extract.evidenceSupportedClaims ?? []).map((claim) => ({
      statement: claim.statement,
      supportScore: Number(claim.supportScore ?? 0),
      supportRefCount: Array.isArray(claim.support) ? claim.support.length : 0,
      hasCitation: Boolean(claim.citation?.url),
    })),
    insufficientEvidenceClaims: extract.insufficientEvidenceClaims ?? [],
  };
}

async function readTouchBrowserReadView(sample) {
  const stdout = await runShell(
    [
      "target/debug/touch-browser",
      "runtime.readView".startsWith("runtime.") ? "read-view" : "read-view",
      shellEscape(sample.url),
      "--main-only",
      "--allow-domain",
      shellEscape(sample.allowDomain),
    ].join(" "),
  );

  return {
    markdownText: stdout,
  };
}

async function readTouchBrowserCompactView(sample) {
  const stdout = await runShell(
    [
      "target/debug/touch-browser",
      "compact-view",
      shellEscape(sample.url),
      "--allow-domain",
      shellEscape(sample.allowDomain),
    ].join(" "),
  );
  return JSON.parse(stdout);
}

async function readTouchBrowserExtract(sample) {
  const args = [
    "target/debug/touch-browser",
    "extract",
    shellEscape(sample.url),
    "--allow-domain",
    shellEscape(sample.allowDomain),
  ];

  for (const claim of [...sample.positiveClaims, ...sample.negativeClaims]) {
    args.push("--claim", shellEscape(claim));
  }

  const stdout = await runShell(args.join(" "));
  const parsed = JSON.parse(stdout);
  return parsed.extract.output;
}

async function fetchHtml(url) {
  try {
    const response = await fetch(url, {
      redirect: "follow",
      headers: {
        "user-agent": "touch-browser/0.1 comparison benchmark",
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

function htmlToMarkdownBaseline(html, sourceUrl) {
  let mainHtml = extractMainContentHtml(html);
  mainHtml = mainHtml
    .replace(/<(script|style|noscript|svg)[\s\S]*?<\/\1>/gi, " ")
    .replace(/<(nav|header|footer|aside)[\s\S]*?<\/\1>/gi, " ")
    .replace(/<br\s*\/?>/gi, "\n")
    .replace(
      /<\/(p|div|section|article|main|ul|ol|li|table|tr|td|th|blockquote)>/gi,
      "\n",
    );

  for (let level = 6; level >= 1; level -= 1) {
    const pattern = new RegExp(
      `<h${level}[^>]*>([\\s\\S]*?)<\\/h${level}>`,
      "gi",
    );
    mainHtml = mainHtml.replace(pattern, (_, inner) => {
      const text = normalizeText(stripHtml(inner));
      return text ? `\n${"#".repeat(level)} ${text}\n` : "\n";
    });
  }

  mainHtml = mainHtml.replace(
    /<a\b[^>]*href=(["'])(.*?)\1[^>]*>([\s\S]*?)<\/a>/gi,
    (_, __, href, inner) => {
      const text = normalizeText(stripHtml(inner));
      return text ? `[${text}](${href})` : "";
    },
  );
  mainHtml = mainHtml.replace(/<li[^>]*>([\s\S]*?)<\/li>/gi, (_, inner) => {
    const text = normalizeText(stripHtml(inner));
    return text ? `\n- ${text}\n` : "\n";
  });

  const markdown = decodeHtmlEntities(stripHtmlPreservingMarkdown(mainHtml))
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .join("\n\n");

  return markdown || `Source: ${sourceUrl}`;
}

function extractMainContentHtml(html) {
  const mainMatch = html.match(/<main\b[^>]*>([\s\S]*?)<\/main>/i);
  if (mainMatch?.[1]) {
    return mainMatch[1];
  }

  const articleMatch = html.match(/<article\b[^>]*>([\s\S]*?)<\/article>/i);
  if (articleMatch?.[1]) {
    return articleMatch[1];
  }

  const bodyMatch = html.match(/<body\b[^>]*>([\s\S]*?)<\/body>/i);
  return bodyMatch?.[1] ?? html;
}

function stripHtmlPreservingMarkdown(input) {
  return input.replace(/<(?!\/?(?:#|\[|\]))[^>]+>/g, " ");
}

function decodeHtmlEntities(text) {
  return text
    .replace(/&nbsp;/gi, " ")
    .replace(/&amp;/gi, "&")
    .replace(/&lt;/gi, "<")
    .replace(/&gt;/gi, ">")
    .replace(/&quot;/gi, '"')
    .replace(/&#39;/gi, "'");
}

function evaluateClaimAgainstText(statement, text) {
  const normalizedText = normalizeText(text);
  const claimTokens = tokenizeSignificant(statement);
  const anchor = anchorTokens(claimTokens);
  const qualifiers = qualifierTokens(statement);
  const supportTokens = tokenizeSignificant(normalizedText);
  const supportAllTokens = tokenizeAll(normalizedText);
  const anchorCoverage = anchor.length
    ? coverageRatio(anchor, supportTokens)
    : 1;
  const qualifierCoverage = qualifiers.length
    ? coverageRatio(qualifiers, supportAllTokens)
    : 1;

  return {
    supported:
      anchorCoverage >= requiredAnchorCoverage(anchor.length) &&
      qualifierCoverage >= 1,
    anchorCoverage: roundTo(anchorCoverage, 2),
    qualifierCoverage: roundTo(qualifierCoverage, 2),
  };
}

function normalizeClaimText(text) {
  return String(text)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim()
    .replace(/\s+/g, " ");
}

function stemToken(token) {
  let stemmed = token;
  for (const suffix of ["ing", "ed", "ly", "es", "s"]) {
    if (stemmed.length > suffix.length + 2 && stemmed.endsWith(suffix)) {
      stemmed = stemmed.slice(0, -suffix.length);
      break;
    }
  }
  return stemmed;
}

function tokenizeAll(text) {
  return [
    ...new Set(
      normalizeClaimText(text).split(" ").filter(Boolean).map(stemToken),
    ),
  ];
}

function tokenizeSignificant(text) {
  return tokenizeAll(text).filter(
    (token) =>
      /^\d+$/.test(token) || (token.length >= 3 && !STOP_WORDS.has(token)),
  );
}

function anchorTokens(tokens) {
  return tokens.filter(
    (token) =>
      token.length >= 5 &&
      !ANCHOR_STOP_WORDS.has(token) &&
      !QUALIFIER_TOKENS.has(token),
  );
}

function qualifierTokens(text) {
  return tokenizeAll(text).filter((token) => QUALIFIER_TOKENS.has(token));
}

function coverageRatio(claimTokens, supportTokens) {
  if (claimTokens.length === 0) {
    return 1;
  }
  const matched = claimTokens.filter((claimToken) =>
    supportTokens.some((supportToken) => tokensMatch(claimToken, supportToken)),
  ).length;
  return matched / claimTokens.length;
}

function requiredAnchorCoverage(anchorCount) {
  if (anchorCount === 0) return 0;
  if (anchorCount <= 2) return 1;
  if (anchorCount === 3) return 0.67;
  return 0.6;
}

function tokensMatch(left, right) {
  return (
    left === right ||
    (left.length >= 4 && right.startsWith(left)) ||
    (right.length >= 4 && left.startsWith(right))
  );
}

const STOP_WORDS = new Set([
  "the",
  "and",
  "for",
  "with",
  "that",
  "this",
  "from",
  "into",
  "your",
  "must",
  "now",
  "are",
  "all",
  "per",
  "there",
  "page",
  "include",
  "includes",
  "includ",
  "contain",
  "contains",
  "list",
  "built",
  "flow",
  "runtime",
  "plan",
  "touch",
  "browser",
]);

const ANCHOR_STOP_WORDS = new Set([
  "support",
  "avail",
  "available",
  "provid",
  "service",
  "system",
  "platform",
  "module",
  "interface",
]);

const QUALIFIER_TOKENS = new Set([
  "all",
  "every",
  "fully",
  "native",
  "global",
  "worldwide",
  "only",
  "always",
  "never",
  "entire",
  "prior",
]);

await main();
