import { execFileSync } from "node:child_process";
import { accessSync, constants as fsConstants } from "node:fs";
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
const SAFE_GIT_EXECUTABLES = [
  "/usr/bin/git",
  "/opt/homebrew/bin/git",
  "/usr/local/bin/git",
];

async function main() {
  const markdownFiles = listTrackedMarkdownFiles();
  const externalLinks = new Map();
  const { anchorFailures, relativeFailures } =
    await collectMarkdownFileFailures(markdownFiles, externalLinks);

  const externalResults = await Promise.all(
    [...externalLinks.keys()].map((url) =>
      checkExternalUrl(url, externalLinks.get(url)?.sourceFiles ?? []),
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
  return execFileSync(resolveGitExecutable(), ["ls-files", "*.md"], {
    cwd: repoRoot,
    encoding: "utf8",
  })
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .sort();
}

function stripCodeFences(markdown) {
  const cleanedLines = [];
  let insideFence = false;

  for (const line of markdown.split("\n")) {
    if (line.trimStart().startsWith("```")) {
      insideFence = !insideFence;
      continue;
    }

    if (!insideFence) {
      cleanedLines.push(line);
    }
  }

  return cleanedLines.join("\n");
}

function extractMarkdownLinks(markdown) {
  const results = [];

  for (let index = 0; index < markdown.length; index += 1) {
    const labelStart =
      markdown[index] === "!" && markdown[index + 1] === "["
        ? index + 1
        : index;
    if (markdown[labelStart] !== "[") {
      continue;
    }

    const labelEnd = markdown.indexOf("]", labelStart + 1);
    if (labelEnd === -1 || markdown[labelEnd + 1] !== "(") {
      continue;
    }

    const hrefEnd = markdown.indexOf(")", labelEnd + 2);
    if (hrefEnd === -1) {
      continue;
    }

    const href = markdown.slice(labelEnd + 2, hrefEnd).trim();
    if (href && !href.includes(" ")) {
      results.push(href);
    }

    index = hrefEnd;
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
  for (const line of content.split("\n")) {
    const heading = parseMarkdownHeading(line);
    if (!heading) {
      continue;
    }
    slugs.add(slugifyHeading(heading));
  }
  return slugs.has(anchor);
}

function slugifyHeading(heading) {
  const tokens = [];
  let current = "";

  for (const char of heading.toLowerCase()) {
    if (isAsciiAlphaNumeric(char)) {
      current += char;
      continue;
    }

    if (current) {
      tokens.push(current);
      current = "";
    }

    if (char === "&") {
      tokens.push("and");
    }
  }

  if (current) {
    tokens.push(current);
  }

  return tokens.join("-");
}

function normalizeRepoRelativePath(relativePath) {
  return relativePath.split(path.sep).join("/");
}

function isGeneratedScenarioArtifact(relativePath) {
  const normalized = normalizeRepoRelativePath(relativePath);
  return (
    normalized.startsWith("fixtures/scenarios/") &&
    (normalized.endsWith("/report.json") ||
      normalized.endsWith("/summary.json"))
  );
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

async function collectMarkdownFileFailures(markdownFiles, externalLinks) {
  const relativeFailures = [];
  const anchorFailures = [];

  for (const relativeFile of markdownFiles) {
    const fileFailures = await collectFileFailures(relativeFile, externalLinks);
    relativeFailures.push(...fileFailures.relativeFailures);
    anchorFailures.push(...fileFailures.anchorFailures);
  }

  return { relativeFailures, anchorFailures };
}

async function collectFileFailures(relativeFile, externalLinks) {
  const sourcePath = path.join(repoRoot, relativeFile);
  const content = stripCodeFences(await readFile(sourcePath, "utf8"));
  const links = extractMarkdownLinks(content);
  const relativeFailures = [];
  const anchorFailures = [];

  for (const href of links) {
    if (shouldSkipHref(href)) {
      continue;
    }

    if (isExternalHref(href)) {
      recordExternalLink(externalLinks, href, relativeFile);
      continue;
    }

    const target = resolveLocalLinkTarget(sourcePath, href);
    if (!(await pathExists(target.resolvedPath))) {
      if (!isGeneratedScenarioArtifact(target.relativeTarget)) {
        relativeFailures.push({
          sourceFile: relativeFile,
          href,
          resolvedTarget: normalizeRepoRelativePath(target.relativeTarget),
        });
      }
      continue;
    }

    if (
      target.anchor &&
      isMarkdownPath(target.resolvedPath) &&
      !(await markdownFileContainsAnchor(target.resolvedPath, target.anchor))
    ) {
      anchorFailures.push({
        sourceFile: relativeFile,
        href,
        resolvedTarget: normalizeRepoRelativePath(target.relativeTarget),
        anchor: target.anchor,
      });
    }
  }

  return { relativeFailures, anchorFailures };
}

function shouldSkipHref(href) {
  const scheme = extractHrefScheme(href);
  return scheme === "mailto" || scheme === "javascript";
}

function isExternalHref(href) {
  const scheme = extractHrefScheme(href);
  return scheme === "http" || scheme === "https";
}

function extractHrefScheme(href) {
  const separatorIndex = href.indexOf(":");
  if (separatorIndex <= 0) {
    return undefined;
  }

  return href.slice(0, separatorIndex).toLowerCase();
}

function resolveLocalLinkTarget(sourcePath, href) {
  const [targetPathRaw, anchor] = href.split("#", 2);
  const resolvedPath = targetPathRaw
    ? path.resolve(path.dirname(sourcePath), targetPathRaw)
    : sourcePath;

  return {
    resolvedPath,
    relativeTarget: path.relative(repoRoot, resolvedPath),
    anchor,
  };
}

function recordExternalLink(externalLinks, href, relativeFile) {
  externalLinks.set(href, {
    url: href,
    sourceFiles: [
      ...new Set([
        ...(externalLinks.get(href)?.sourceFiles ?? []),
        relativeFile,
      ]),
    ],
  });
}

function isMarkdownPath(filePath) {
  return filePath.endsWith(".md") || filePath.endsWith(".markdown");
}

function parseMarkdownHeading(line) {
  let index = 0;
  while (index < line.length && line[index] === "#") {
    index += 1;
  }

  if (index === 0 || index > 6 || line[index] !== " ") {
    return undefined;
  }

  return line.slice(index + 1).trim() || undefined;
}

function isAsciiAlphaNumeric(char) {
  return isAsciiDigit(char) || (char >= "a" && char <= "z");
}

function isAsciiDigit(char) {
  return char >= "0" && char <= "9";
}

function resolveGitExecutable() {
  for (const candidate of SAFE_GIT_EXECUTABLES) {
    try {
      accessSync(candidate, fsConstants.X_OK);
      return candidate;
    } catch {
      // Try the next well-known executable path.
    }
  }

  throw new Error(
    "Could not locate git in the allowlisted executable paths for link integrity checks.",
  );
}

await main();
