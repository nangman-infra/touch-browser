import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { spawnShell } from "./lib/shell-command.mjs";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "..");

const scenarios = [
  {
    name: "policy-static-docs",
    target: "fixture://research/static-docs/getting-started",
  },
  {
    name: "policy-hostile-fake-system",
    target: "fixture://research/hostile/fake-system-message",
  },
  {
    name: "policy-navigation-captcha",
    target: "fixture://research/navigation/browser-captcha-checkpoint",
  },
  {
    name: "policy-navigation-mfa",
    target: "fixture://research/navigation/browser-mfa-challenge",
  },
  {
    name: "policy-navigation-high-risk",
    target: "fixture://research/navigation/browser-high-risk-checkout",
  },
];

async function main() {
  for (const scenario of scenarios) {
    const output = await runPolicyCommand(scenario.target);
    const scenarioDir = path.join(
      repoRoot,
      "fixtures",
      "scenarios",
      scenario.name,
    );

    await mkdir(scenarioDir, { recursive: true });
    await writeFile(
      path.join(scenarioDir, "report.json"),
      `${JSON.stringify(JSON.parse(output), null, 2)}\n`,
    );
  }
}

async function runPolicyCommand(target) {
  const child = spawnShell(
    `cargo run -q -p touch-browser-cli -- policy '${target}'`,
    {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  const stdout = [];
  const stderr = [];
  child.stdout.on("data", (chunk) => stdout.push(chunk));
  child.stderr.on("data", (chunk) => stderr.push(chunk));

  const exitCode = await new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });

  if (exitCode !== 0) {
    throw new Error(Buffer.concat(stderr).toString("utf8"));
  }

  return Buffer.concat(stdout).toString("utf8").trim();
}

await main();
