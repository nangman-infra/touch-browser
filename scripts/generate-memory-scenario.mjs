import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { spawnShell } from "./lib/shell-command.mjs";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "..");

const scenarios = [
  { actions: 20, name: "memory-20-step" },
  { actions: 50, name: "memory-50-step" },
  { actions: 100, name: "memory-100-step" },
];

async function main() {
  for (const scenario of scenarios) {
    const output = await runMemoryScenarioExample(scenario.actions);
    const parsed = JSON.parse(output);
    const scenarioDir = path.join(
      repoRoot,
      "fixtures",
      "scenarios",
      scenario.name,
    );

    await mkdir(scenarioDir, { recursive: true });
    await writeFile(
      path.join(scenarioDir, "summary.json"),
      `${JSON.stringify(parsed, null, 2)}\n`,
    );
  }
}

async function runMemoryScenarioExample(actions) {
  const child = spawnShell(
    `TOUCH_BROWSER_MEMORY_ACTIONS=${actions} cargo run -q -p touch-browser-runtime --example run_memory_session`,
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
