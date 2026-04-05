import { execFileSync } from "node:child_process";
import { access, readFile } from "node:fs/promises";
import path from "node:path";

import {
  repoRoot,
  roundTo,
  runShell,
  shellEscape,
} from "./lib/live-sample-server.mjs";
import { writeRepoJson } from "./lib/scenario-files.mjs";

const reportPath = "fixtures/scenarios/doc-link-integrity/report.json";
const timeoutMs = 15_000;
async function main() {
  const markdownFiles = listTrackedMarkdownFiles();
  const relativeFailures = [];
  const anchorFailures = [];
  const externalLinks = new Map();

  for (const relativeFile of markdownFiles) {
    const sourcePath = path.join(repoRoot, relativeFile);
    const raw = await readFile(sourcePath, "utf8");
    const content = stripCodeFences(raw);
    const links = extractMarkdownLinks(content);

    for (const href of links) {
      if (href.startsWith("mailto:") || href.startsWith("javascript:")) {
        continue;
      }

      if (href.startsWith("http://") || href.startsWith("https://")) {
        externalLinks.set(href, {
          url: href,
          sourceFiles: [
            ...new Set([
              ...(externalLinks.get(href)?.sourceFiles ?? []),
              relativeFile,
            ]),
          ],
        });
        continue;
      }

      const [targetPathRaw, anchor] = href.split("#", 2);
      const resolvedPath = targetPathRaw
        ? path.resolve(path.dirname(sourcePath), targetPathRaw)
        : sourcePath;
      const relativeTarget = path.relative(repoRoot, resolvedPath);

      if (!(await pathExists(resolvedPath))) {
        relativeFailures.push({
          sourceFile: relativeFile,
          href,
          resolvedTarget: normalizeRepoRelativePath(relativeTarget),
        });
        continue;
      }

      if (anchor && /\.(md|markdown)$/i.test(resolvedPath)) {
        const anchorValid = await markdownFileContainsAnchor(
          resolvedPath,
          anchor,
        );
        if (!anchorValid) {
          anchorFailures.push({
            sourceFile: relativeFile,
            href,
            resolvedTarget: normalizeRepoRelativePath(relativeTarget),
            anchor,
          });
        }
      }
    }
  }

  const externalResults = await Promise.all(
    [...externalLinks.keys()].map((url) =>
      checkExternalUrl(url, externalLinks.get(url).sourceFiles),
    ),
  );

  const externalFailures = externalResults.filter((result) => !result.ok);
  const report = {
    status:
      relativeFailures.length === 0 &&
      anchorFailures.length === 0 &&
      externalFailures.length === 0
        ? "ok"
        : "failed",
    checkedAt: new Date().toISOString(),
    markdownFileCount: markdownFiles.length,
    relativeFailureCount: relativeFailures.length,
    anchorFailureCount: anchorFailures.length,
    externalLinkCount: externalLinks.size,
    externalCheckedCount: externalResults.length,
    externalSuccessRate: roundTo(
      externalResults.length === 0
        ? 1
        : (externalResults.length - externalFailures.length) /
            externalResults.length,
      2,
    ),
    relativeFailures,
    anchorFailures,
    externalFailures,
    externalChecks: externalResults,
  };

  await writeRepoJson(reportPath, report);
}

function listTrackedMarkdownFiles() {
  return execFileSync("git", ["ls-files", "*.md"], {
    cwd: repoRoot,
    encoding: "utf8",
  })
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .sort();
}

function stripCodeFences(markdown) {
  return markdown.replace(/```[\s\S]*?```/g, "");
}

function extractMarkdownLinks(markdown) {
  const results = [];
  const linkPattern = /!?\[[^\]]*?\]\(([^)\s]+(?:\#[^)\\s]+)?)\)/g;
  for (const match of markdown.matchAll(linkPattern)) {
    if (match[1]) {
      results.push(match[1].trim());
    }
  }
  return results;
}

async function pathExists(targetPath) {
  try {
    await access(targetPath);
    return true;
  } catch {
    return false;
  }
}

async function markdownFileContainsAnchor(filePath, anchor) {
  const content = stripCodeFences(await readFile(filePath, "utf8"));
  const slugs = new Set();
  const headingMatches = content.matchAll(/^(#{1,6})\s+(.+)$/gm);
  for (const match of headingMatches) {
    const heading = match[2]?.trim();
    if (!heading) {
      continue;
    }
    slugs.add(slugifyHeading(heading));
  }
  return slugs.has(anchor);
}

function slugifyHeading(heading) {
  return heading
    .toLowerCase()
    .replace(/[`*_~()[\]{}<>]/g, "")
    .replace(/&/g, " and ")
    .replace(/[^a-z0-9\s-]/g, "")
    .trim()
    .replace(/\s+/g, "-")
    .replace(/-+/g, "-");
}

function normalizeRepoRelativePath(relativePath) {
  return relativePath.split(path.sep).join("/");
}

async function checkExternalUrl(url, sourceFiles) {
  try {
    const stdout = await runShell(
      [
        "curl",
        "-ILsS",
        "--max-time",
        String(Math.ceil(timeoutMs / 1000)),
        "-A",
        shellEscape("touch-browser-doc-link-check/0.1"),
        "-o",
        "/dev/null",
        "-w",
        shellEscape("%{http_code} %{url_effective}"),
        shellEscape(url),
      ].join(" "),
    );
    const [statusText, ...finalUrlParts] = stdout.trim().split(" ");
    const status = Number(statusText);

    return {
      url,
      sourceFiles,
      ok: status >= 200 && status < 400,
      status,
      finalUrl: finalUrlParts.join(" "),
    };
  } catch (error) {
    return {
      url,
      sourceFiles,
      ok: false,
      error: error instanceof Error ? error.message : String(error),
    };
  }
}

await main();
