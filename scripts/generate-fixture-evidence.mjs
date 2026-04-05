import { spawn } from "node:child_process";
import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "..");
const fixturesRoot = path.join(repoRoot, "fixtures", "research");

async function main() {
  const fixtureMetadataPaths = await listFixtureMetadataPaths(fixturesRoot);
  const writtenEvidencePaths = [];

  for (const metadataPath of fixtureMetadataPaths) {
    const fixture = JSON.parse(await readFile(metadataPath, "utf8"));
    const expectedEvidencePath = path.join(
      repoRoot,
      fixture.expectedEvidencePath,
    );
    const evidenceJson = await renderEvidence(metadataPath);

    await mkdir(path.dirname(expectedEvidencePath), { recursive: true });
    await writeFile(expectedEvidencePath, evidenceJson);
    writtenEvidencePaths.push(expectedEvidencePath);
  }

  await formatGeneratedEvidence(writtenEvidencePaths);
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

async function renderEvidence(metadataPath) {
  const child = spawn(
    "zsh",
    [
      "-lic",
      [
        "cargo run -q -p touch-browser-evidence --example render_fixture_evidence",
        shellEscape(metadataPath),
      ].join(" "),
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
    throw new Error(
      `Failed to render evidence for ${metadataPath}: ${Buffer.concat(stderr).toString("utf8")}`,
    );
  }

  return `${Buffer.concat(stdout).toString("utf8").trim()}\n`;
}

function shellEscape(value) {
  return `'${String(value).replaceAll("'", "'\\''")}'`;
}

async function formatGeneratedEvidence(filePaths) {
  if (filePaths.length === 0) {
    return;
  }

  const child = spawn(
    "zsh",
    [
      "-lic",
      [
        "pnpm exec biome format --write",
        ...filePaths.map((filePath) => shellEscape(filePath)),
      ].join(" "),
    ],
    {
      cwd: repoRoot,
      stdio: ["ignore", "pipe", "pipe"],
    },
  );

  const stderr = [];
  child.stderr.on("data", (chunk) => stderr.push(chunk));

  const exitCode = await new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });

  if (exitCode !== 0) {
    throw new Error(
      `Failed to format generated evidence: ${Buffer.concat(stderr).toString("utf8")}`,
    );
  }
}

await main();
