import { spawn } from "node:child_process";
import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "..");
const scenarioDir = path.join(
  repoRoot,
  "fixtures",
  "scenarios",
  "read-only-pricing",
);

async function main() {
  const output = await runScenarioExample();
  const parsed = JSON.parse(output);

  await mkdir(scenarioDir, { recursive: true });
  await writeFile(
    path.join(scenarioDir, "session-state.json"),
    `${JSON.stringify(parsed.sessionState, null, 2)}\n`,
  );
  await writeFile(
    path.join(scenarioDir, "replay-transcript.json"),
    `${JSON.stringify(parsed.replayTranscript, null, 2)}\n`,
  );
}

async function runScenarioExample() {
  const child = spawn(
    "zsh",
    [
      "-lic",
      "cargo run -q -p touch-browser-runtime --example run_fixture_session",
    ],
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
