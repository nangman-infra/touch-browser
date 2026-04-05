import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import {
  ensureCliBuilt,
  liveSamples,
  renderCompactSnapshot,
  repoRoot,
  runShell,
  shellEscape,
  withLiveSampleServer,
} from "./lib/live-sample-server.mjs";

const outputDir = path.join(
  repoRoot,
  "fixtures",
  "scenarios",
  "latency-cost-metrics",
);

async function main() {
  await ensureCliBuilt();

  const report = await withLiveSampleServer(async ({ baseUrl }) => {
    const startUrl = `${baseUrl}/start`;
    const pricingUrl = `${baseUrl}/pricing`;
    const fixtureTarget = "fixture://research/static-docs/getting-started";

    const fixtureCompact = await timedCli(["compact-view", fixtureTarget]);
    const liveOpen = await timedCli([
      "open",
      startUrl,
      "--allow-domain",
      "127.0.0.1",
    ]);
    const browserOpen = await timedCli([
      "open",
      startUrl,
      "--browser",
      "--allow-domain",
      "127.0.0.1",
    ]);
    const liveExtract = await timedCli([
      "extract",
      pricingUrl,
      "--allow-domain",
      "127.0.0.1",
      "--claim",
      "Starter plan costs $29 per month.",
    ]);

    const rawPricingTokens = countTokens(
      liveSamples.find((sample) => sample.id === "pricing").html,
    );
    const compactPricingTokens = countTokens(
      renderCompactSnapshot(liveExtract.open.output),
    );

    return {
      fixtureCompactMs: fixtureCompact.elapsedMs,
      liveOpenMs: liveOpen.elapsedMs,
      browserOpenMs: browserOpen.elapsedMs,
      liveExtractMs: liveExtract.elapsedMs,
      compactTokenCostRatio: roundTo(
        compactPricingTokens / Math.max(rawPricingTokens, 1),
        2,
      ),
      browserLatencyMultiplier: roundTo(
        browserOpen.elapsedMs / Math.max(liveOpen.elapsedMs, 1),
        2,
      ),
      timings: {
        fixtureCompact: fixtureCompact.elapsedMs,
        liveOpen: liveOpen.elapsedMs,
        browserOpen: browserOpen.elapsedMs,
        liveExtract: liveExtract.elapsedMs,
      },
    };
  });

  await mkdir(outputDir, { recursive: true });
  await writeFile(
    path.join(outputDir, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}

async function timedCli(args) {
  const startedAt = performance.now();
  const stdout = await runShell(
    `target/debug/touch-browser ${args.map(shellEscape).join(" ")}`,
  );
  return {
    elapsedMs: roundTo(performance.now() - startedAt, 2),
    ...JSON.parse(stdout),
  };
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
