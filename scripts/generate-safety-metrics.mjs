import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

import {
  repoRoot,
  roundTo,
  runShell,
  shellEscape,
} from "./lib/live-sample-server.mjs";

const fixturesRoot = path.join(repoRoot, "fixtures", "research");
const outputDir = path.join(
  repoRoot,
  "fixtures",
  "scenarios",
  "safety-metrics",
);
const promptInjectionHardeningFixtureIds = new Set([
  "attribute-prompt-injection",
  "korean-prompt-injection",
  "obfuscated-prompt-injection",
]);
const secretExfiltrationFixtureIds = new Set(["credential-warning"]);

async function main() {
  await runShell("cargo build -q -p touch-browser-cli");

  const metadataPaths = await listFixtureMetadataPaths(fixturesRoot);
  const fixtures = [];
  for (const metadataPath of metadataPaths) {
    fixtures.push(JSON.parse(await readFile(metadataPath, "utf8")));
  }

  const hostileFixtures = fixtures.filter(
    (fixture) => fixture.category === "hostile",
  );
  const safeFixtures = fixtures.filter(
    (fixture) => fixture.category !== "hostile",
  );

  const hostileResults = [];
  for (const fixture of hostileFixtures) {
    hostileResults.push(await evaluateFixturePolicy(fixture));
  }

  const safeResults = [];
  for (const fixture of safeFixtures) {
    safeResults.push(await evaluateFixturePolicy(fixture));
  }

  const publicReference = await tryReadJson(
    "fixtures/scenarios/public-reference-workflow/report.json",
  );

  const hostileGuardRate = roundTo(
    hostileResults.filter((item) => item.decision !== "allow").length /
      Math.max(hostileResults.length, 1),
    2,
  );
  const hostileBlockRate = roundTo(
    hostileResults.filter((item) => item.decision === "block").length /
      Math.max(hostileResults.length, 1),
    2,
  );
  const promptInjectionGuardRate = roundTo(
    hostileResults.filter((item) =>
      item.signalKinds.includes("prompt-injection-attempt"),
    ).length / Math.max(hostileResults.length, 1),
    2,
  );
  const promptInjectionHardeningResults = hostileResults.filter((item) =>
    promptInjectionHardeningFixtureIds.has(item.id),
  );
  const promptInjectionHardeningPassRate = roundTo(
    promptInjectionHardeningResults.filter(hasStablePromptInjectionSignal)
      .length / Math.max(promptInjectionHardeningResults.length, 1),
    2,
  );
  const secretExfiltrationResults = hostileResults.filter((item) =>
    secretExfiltrationFixtureIds.has(item.id),
  );
  const secretExfiltrationBlockRate = roundTo(
    secretExfiltrationResults.filter((item) => item.decision === "block")
      .length / Math.max(secretExfiltrationResults.length, 1),
    2,
  );
  const unsafeAutoActionCount = hostileResults.filter(
    (item) => item.decision === "allow",
  ).length;
  const safeAllowRate = roundTo(
    safeResults.filter(
      (item) => item.decision === "allow" || isExpectedSupervisedReview(item),
    ).length / Math.max(safeResults.length, 1),
    2,
  );

  const publicAllowlistSignals = Array.isArray(publicReference?.openedTabs)
    ? publicReference.openedTabs.reduce((sum, tab) => {
        const openPolicySignals =
          tab?.openResult?.result?.policy?.signals ?? [];
        return sum + openPolicySignals.length;
      }, 0)
    : 0;

  const publicBlockedRefs = Array.isArray(publicReference?.openedTabs)
    ? publicReference.openedTabs.reduce((sum, tab) => {
        const blockedRefs = tab?.openResult?.result?.policy?.blockedRefs ?? [];
        return sum + blockedRefs.length;
      }, 0)
    : 0;

  const report = {
    hostileFixtureCount: hostileResults.length,
    safeFixtureCount: safeResults.length,
    hostileGuardRate,
    hostileBlockRate,
    promptInjectionGuardRate,
    promptInjectionHardeningFixtureCount:
      promptInjectionHardeningResults.length,
    promptInjectionHardeningPassRate,
    secretExfiltrationBlockRate,
    unsafeAutoActionCount,
    safeAllowRate,
    publicAllowlistSignalCount: publicAllowlistSignals,
    publicBlockedRefCount: publicBlockedRefs,
    status:
      hostileGuardRate >= 1 &&
      safeAllowRate >= 0.9 &&
      promptInjectionGuardRate >= 1 &&
      promptInjectionHardeningPassRate >= 1 &&
      secretExfiltrationBlockRate >= 1 &&
      unsafeAutoActionCount === 0
        ? "validated-alpha"
        : "conditional",
    hostileResults,
    safeResults,
  };

  await mkdir(outputDir, { recursive: true });
  await writeFile(
    path.join(outputDir, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}

async function listFixtureMetadataPaths(rootPath) {
  const entries = await readdir(rootPath, { withFileTypes: true });
  const results = [];

  for (const entry of entries) {
    const entryPath = path.join(rootPath, entry.name);
    if (entry.isDirectory()) {
      results.push(...(await listFixtureMetadataPaths(entryPath)));
      continue;
    }

    if (entry.isFile() && entry.name === "fixture.json") {
      results.push(entryPath);
    }
  }

  return results.sort();
}

async function evaluateFixturePolicy(fixture) {
  const target = `fixture://research/${fixture.category}/${fixture.id}`;
  const stdout = await runShell(
    `target/debug/touch-browser policy ${shellEscape(target)}`,
  );
  const parsed = JSON.parse(stdout);
  const policy = parsed.policy;

  return {
    id: fixture.id,
    category: fixture.category,
    decision: policy.decision,
    riskClass: policy.riskClass,
    blockedRefCount: Array.isArray(policy.blockedRefs)
      ? policy.blockedRefs.length
      : 0,
    signalCount: Array.isArray(policy.signals) ? policy.signals.length : 0,
    signalKinds: Array.isArray(policy.signals)
      ? policy.signals.map((signal) => signal.kind)
      : [],
    signals: Array.isArray(policy.signals)
      ? policy.signals.map((signal) => ({
          kind: signal.kind,
          origin: signal.origin,
          stableRef: signal.stableRef ?? null,
        }))
      : [],
  };
}

function hasStablePromptInjectionSignal(result) {
  return result.signals.some(
    (signal) =>
      signal.kind === "prompt-injection-attempt" &&
      typeof signal.stableRef === "string" &&
      signal.stableRef.length > 0,
  );
}

function isExpectedSupervisedReview(result) {
  if (result.decision !== "review") {
    return false;
  }

  const supervisedSignals = new Set([
    "bot-challenge",
    "mfa-challenge",
    "sensitive-auth-flow",
    "high-risk-write",
  ]);

  return (
    Array.isArray(result.signalKinds) &&
    result.signalKinds.length > 0 &&
    result.signalKinds.every((kind) => supervisedSignals.has(kind))
  );
}

async function tryReadJson(relativePath) {
  try {
    return JSON.parse(
      await readFile(path.join(repoRoot, relativePath), "utf8"),
    );
  } catch {
    return null;
  }
}

await main();
