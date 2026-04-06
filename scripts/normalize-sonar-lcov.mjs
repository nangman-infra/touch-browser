import fs from "node:fs";
import path from "node:path";

function toPosixPath(value) {
  return value.split(path.sep).join(path.posix.sep);
}

function main() {
  const [, , reportArg, sourceRootArg] = process.argv;

  if (!reportArg || !sourceRootArg) {
    throw new Error(
      "Usage: node scripts/normalize-sonar-lcov.mjs <lcov-path> <source-root>",
    );
  }

  const repoRoot = process.cwd();
  const reportPath = path.resolve(repoRoot, reportArg);
  const sourceRoot = path.resolve(repoRoot, sourceRootArg);
  const report = fs.readFileSync(reportPath, "utf8");

  const normalized = report
    .split("\n")
    .map((line) => {
      if (!line.startsWith("SF:")) {
        return line;
      }

      const originalPath = line.slice(3);
      if (!originalPath || path.isAbsolute(originalPath)) {
        return line;
      }

      const repoRelativePath = path.resolve(repoRoot, originalPath);
      if (fs.existsSync(repoRelativePath)) {
        return `SF:${toPosixPath(path.relative(repoRoot, repoRelativePath))}`;
      }

      const sourceRelativePath = path.resolve(sourceRoot, originalPath);
      if (fs.existsSync(sourceRelativePath)) {
        return `SF:${toPosixPath(path.relative(repoRoot, sourceRelativePath))}`;
      }

      return line;
    })
    .join("\n");

  fs.writeFileSync(reportPath, normalized);
}

main();
