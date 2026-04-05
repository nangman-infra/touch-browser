import { access, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

import { repoRoot } from "./live-sample-server.mjs";

export function resolveRepoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

export async function readRepoJson(relativePath) {
  return JSON.parse(await readFile(resolveRepoPath(relativePath), "utf8"));
}

export async function tryReadRepoJson(relativePath) {
  try {
    return await readRepoJson(relativePath);
  } catch {
    return null;
  }
}

export async function writeRepoJson(relativePath, payload) {
  const outputPath = resolveRepoPath(relativePath);
  await mkdir(path.dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(payload, null, 2)}\n`);
}

export async function allRepoPathsExist(relativePaths) {
  const results = await Promise.all(
    relativePaths.map(async (relativePath) => {
      try {
        await access(resolveRepoPath(relativePath));
        return true;
      } catch {
        return false;
      }
    }),
  );

  return results.every(Boolean);
}

export async function listRepoFilesNamed(relativeRoot, fileName) {
  return await listFilesNamed(resolveRepoPath(relativeRoot), fileName);
}

async function listFilesNamed(rootPath, fileName) {
  const entries = await readdir(rootPath, { withFileTypes: true });
  const results = [];

  for (const entry of entries) {
    const entryPath = path.join(rootPath, entry.name);
    if (entry.isDirectory()) {
      results.push(...(await listFilesNamed(entryPath, fileName)));
      continue;
    }

    if (entry.isFile() && entry.name === fileName) {
      results.push(entryPath);
    }
  }

  return results.sort();
}
