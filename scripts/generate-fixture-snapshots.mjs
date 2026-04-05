import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

import { spawnShell } from "./lib/shell-command.mjs";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "..");
const fixturesRoot = path.join(repoRoot, "fixtures", "research");

async function main() {
  const fixtureMetadataPaths = await listFixtureMetadataPaths(fixturesRoot);

  for (const metadataPath of fixtureMetadataPaths) {
    const fixture = JSON.parse(await readFile(metadataPath, "utf8"));
    const htmlPath = path.join(repoRoot, fixture.htmlPath);
    const expectedSnapshotPath = path.join(
      repoRoot,
      fixture.expectedSnapshotPath,
    );
    const snapshotJson = await renderSnapshot(htmlPath, fixture.sourceUri);

    await mkdir(path.dirname(expectedSnapshotPath), { recursive: true });
    await writeFile(expectedSnapshotPath, snapshotJson);
  }
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

async function renderSnapshot(htmlPath, sourceUri) {
  const child = spawnShell(
    [
      "cargo run -q -p touch-browser-observation --example render_fixture",
      shellEscape(htmlPath),
      shellEscape(sourceUri),
      "512",
    ].join(" "),
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
    throw new Error(
      `Failed to render snapshot for ${htmlPath}: ${Buffer.concat(stderr).toString("utf8")}`,
    );
  }

  return `${Buffer.concat(stdout).toString("utf8").trim()}\n`;
}

function shellEscape(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

await main();
