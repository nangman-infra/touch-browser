import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import {
  ensureCliBuilt,
  repoRoot,
  runShell,
  shellEscape,
  withLiveSampleServer,
} from "./lib/live-sample-server.mjs";

const outputDir = path.join(
  repoRoot,
  "fixtures",
  "scenarios",
  "live-session-synthesis",
);

async function main() {
  await ensureCliBuilt();

  const report = await withLiveSampleServer(async ({ baseUrl }) => {
    const tempDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-session-"),
    );
    const sessionFile = path.join(tempDir, "session.json");

    try {
      const startUrl = `${baseUrl}/start`;
      const open = await runCli([
        "open",
        startUrl,
        "--browser",
        "--session-file",
        sessionFile,
        "--allow-domain",
        "127.0.0.1",
      ]);
      let currentSnapshot = await runCli([
        "session-snapshot",
        "--session-file",
        sessionFile,
      ]);
      const docsRef = refByText(currentSnapshot.action.output, "Docs page");

      const followDocs = await runCli([
        "follow",
        "--session-file",
        sessionFile,
        "--ref",
        docsRef,
      ]);
      const docsExtract = await runCli([
        "session-extract",
        "--session-file",
        sessionFile,
        "--claim",
        "Semantic snapshots keep stable refs and evidence metadata.",
      ]);

      currentSnapshot = await runCli([
        "session-snapshot",
        "--session-file",
        sessionFile,
      ]);
      const pricingRef = refByText(
        currentSnapshot.action.output,
        "Pricing page",
      );

      const followPricing = await runCli([
        "follow",
        "--session-file",
        sessionFile,
        "--ref",
        pricingRef,
      ]);
      const pricingExtract = await runCli([
        "session-extract",
        "--session-file",
        sessionFile,
        "--claim",
        "Starter plan costs $29 per month.",
        "--claim",
        "Enterprise plan costs $9 per month.",
      ]);

      const synthesis = await runCli([
        "session-synthesize",
        "--session-file",
        sessionFile,
        "--note-limit",
        "8",
      ]);
      const compact = await runCli([
        "session-compact",
        "--session-file",
        sessionFile,
      ]);
      const replay = await runCli([
        "browser-replay",
        "--session-file",
        sessionFile,
      ]);

      return {
        sessionFile,
        open,
        followDocs,
        docsExtract,
        followPricing,
        pricingExtract,
        synthesis,
        compact,
        replay,
      };
    } finally {
      await runCli(["session-close", "--session-file", sessionFile]).catch(
        () => null,
      );
      await rm(tempDir, { recursive: true, force: true });
    }
  });

  await mkdir(outputDir, { recursive: true });
  await writeFile(
    path.join(outputDir, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}

function refByText(snapshot, text) {
  const block = snapshot.blocks.find((candidate) => candidate.text === text);
  if (!block) {
    throw new Error(`Could not find stable ref for text: ${text}`);
  }

  return block.ref;
}

async function runCli(args) {
  const stdout = await runShell(
    `target/debug/touch-browser ${args.map(shellEscape).join(" ")}`,
  );
  return JSON.parse(stdout);
}

await main();
