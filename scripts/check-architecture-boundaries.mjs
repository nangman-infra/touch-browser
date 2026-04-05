import { access, readFile, readdir } from "node:fs/promises";
import path from "node:path";

const requiredDocs = [
  {
    relativePath: "doc/CONTEXT_MAP.md",
    requiredText: [
      "Observation",
      "Evidence",
      "Memory",
      "Policy",
      "Action VM",
      "Playwright Adapter",
      "Contracts",
    ],
  },
  {
    relativePath: "doc/UBIQUITOUS_LANGUAGE.md",
    requiredText: [
      "Snapshot",
      "Stable Ref",
      "Evidence",
      "Claim",
      "Policy",
      "Session",
      "Action Result",
      "Compact View",
    ],
  },
  {
    relativePath: "doc/DDD_COMPLETION_CRITERIA.md",
    requiredText: [
      "1 | Explicit Context Map",
      "6 | Continuous Quality Gate",
      "SonarQube",
      "quality:ci",
      "architecture:check",
    ],
  },
];

const requiredWorkflowPatterns = [
  { label: "quality job", pattern: /quality-checks:/ },
  { label: "quality ci run", pattern: /pnpm run quality:ci/ },
  { label: "sonar reports run", pattern: /pnpm run quality:sonar-reports/ },
  { label: "quality gate wait", pattern: /sonar\.qualitygate\.wait=true/ },
];

const requiredSonarPatterns = [
  { label: "project key", pattern: /^sonar\.projectKey=/m },
  { label: "sources", pattern: /^sonar\.sources=/m },
  { label: "exclusions", pattern: /^sonar\.exclusions=/m },
  {
    label: "rust manifest path",
    pattern: /^sonar\.rust\.cargo\.manifestPaths=/m,
  },
  {
    label: "disable duplicate auto clippy",
    pattern: /^sonar\.rust\.clippy\.enable=false$/m,
  },
  {
    label: "clippy report import",
    pattern: /^sonar\.rust\.clippy\.reportPaths=/m,
  },
];

const applicationForbiddenPatterns = [
  {
    label: "infrastructure coupling",
    pattern: /crate::infrastructure::/g,
  },
  {
    label: "composition root inside application",
    pattern: /default_cli_ports\(/g,
  },
  {
    label: "process spawn in application",
    pattern: /Command::new\(/g,
  },
  {
    label: "stdio handle in application",
    pattern: /Stdio::/g,
  },
  {
    label: "raw json macro in application",
    pattern: /json!\(/g,
  },
  {
    label: "wildcard crate import in application",
    pattern: /use crate::\*;/g,
  },
];

const contractsForbiddenPatterns = [
  {
    label: "public presentation renderer",
    pattern: /pub fn render_[a-zA-Z0-9_]+\s*\(/g,
  },
  {
    label: "public compact ref index renderer",
    pattern: /pub fn compact_ref_index\s*\(/g,
  },
  {
    label: "public navigation ref index renderer",
    pattern: /pub fn navigation_ref_index\s*\(/g,
  },
];

async function collectFiles(directory, predicate) {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const absolutePath = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...(await collectFiles(absolutePath, predicate)));
      continue;
    }
    if (predicate(absolutePath)) {
      files.push(absolutePath);
    }
  }

  return files;
}

function stripCfgTestModules(content) {
  const lines = content.split("\n");
  const kept = [];
  let pendingCfgTest = false;
  let skippingModule = false;
  let braceDepth = 0;

  for (const line of lines) {
    const trimmed = line.trim();

    if (skippingModule) {
      braceDepth += countChar(line, "{");
      braceDepth -= countChar(line, "}");
      if (braceDepth <= 0) {
        skippingModule = false;
        braceDepth = 0;
      }
      continue;
    }

    if (trimmed === "#[cfg(test)]") {
      pendingCfgTest = true;
      continue;
    }

    if (
      pendingCfgTest &&
      /^(pub\([^)]*\)\s+)?mod\s+[A-Za-z0-9_]+\s*\{/.test(trimmed)
    ) {
      skippingModule = true;
      braceDepth = countChar(line, "{") - countChar(line, "}");
      pendingCfgTest = false;
      continue;
    }

    pendingCfgTest = false;
    kept.push(line);
  }

  return kept.join("\n");
}

function countChar(text, char) {
  return [...text].filter((candidate) => candidate === char).length;
}

function collectPatternViolations(content, patterns, relativePath) {
  const violations = [];

  for (const { label, pattern } of patterns) {
    const matches = [...content.matchAll(pattern)];
    for (const match of matches) {
      const line = content.slice(0, match.index).split("\n").length;
      violations.push({
        type: "pattern",
        file: relativePath,
        line,
        label,
        snippet: match[0],
      });
    }
  }

  return violations;
}

async function assertFileContains(
  root,
  relativePath,
  requiredText,
  violations,
) {
  const absolutePath = path.join(root, relativePath);
  await access(absolutePath);
  const content = await readFile(absolutePath, "utf8");

  for (const text of requiredText) {
    if (!content.includes(text)) {
      violations.push({
        type: "missing-text",
        file: relativePath,
        label: `required text: ${text}`,
      });
    }
  }
}

async function main() {
  const root = process.cwd();
  const violations = [];

  for (const { relativePath, requiredText } of requiredDocs) {
    try {
      await assertFileContains(root, relativePath, requiredText, violations);
    } catch (error) {
      violations.push({
        type: "missing-file",
        file: relativePath,
        label: error instanceof Error ? error.message : String(error),
      });
    }
  }

  const applicationFiles = await collectFiles(
    path.join(root, "core/crates/cli/src/application"),
    (absolutePath) => absolutePath.endsWith(".rs"),
  );

  for (const absolutePath of applicationFiles) {
    const relativePath = path.relative(root, absolutePath);
    const rawContent = await readFile(absolutePath, "utf8");
    const scanContent = stripCfgTestModules(rawContent);
    violations.push(
      ...collectPatternViolations(
        scanContent,
        applicationForbiddenPatterns,
        relativePath,
      ),
    );
  }

  const contractsPath = path.join(root, "core/crates/contracts/src/lib.rs");
  const contractsContent = await readFile(contractsPath, "utf8");
  violations.push(
    ...collectPatternViolations(
      contractsContent,
      contractsForbiddenPatterns,
      path.relative(root, contractsPath),
    ),
  );

  const workflowPath = path.join(root, ".github/workflows/sonar.yml");
  try {
    const workflowContent = await readFile(workflowPath, "utf8");
    for (const { label, pattern } of requiredWorkflowPatterns) {
      if (!pattern.test(workflowContent)) {
        violations.push({
          type: "missing-workflow-pattern",
          file: path.relative(root, workflowPath),
          label,
        });
      }
    }
  } catch (error) {
    violations.push({
      type: "missing-file",
      file: path.relative(root, workflowPath),
      label: error instanceof Error ? error.message : String(error),
    });
  }

  const sonarProjectPath = path.join(root, "sonar-project.properties");
  try {
    const sonarProjectContent = await readFile(sonarProjectPath, "utf8");
    for (const { label, pattern } of requiredSonarPatterns) {
      if (!pattern.test(sonarProjectContent)) {
        violations.push({
          type: "missing-sonar-pattern",
          file: path.relative(root, sonarProjectPath),
          label,
        });
      }
    }
  } catch (error) {
    violations.push({
      type: "missing-file",
      file: path.relative(root, sonarProjectPath),
      label: error instanceof Error ? error.message : String(error),
    });
  }

  const packageJsonPath = path.join(root, "package.json");
  const packageJson = JSON.parse(await readFile(packageJsonPath, "utf8"));
  const scripts = packageJson.scripts ?? {};
  const requiredScripts = [
    "architecture:check",
    "quality:ci",
    "quality:sonar-reports",
  ];
  for (const scriptName of requiredScripts) {
    if (typeof scripts[scriptName] !== "string") {
      violations.push({
        type: "missing-script",
        file: path.relative(root, packageJsonPath),
        label: scriptName,
      });
    }
  }
  if (
    typeof scripts.test === "string" &&
    !scripts.test.includes("pnpm run architecture:check")
  ) {
    violations.push({
      type: "missing-test-gate",
      file: path.relative(root, packageJsonPath),
      label: "test script must include architecture:check",
    });
  }

  if (violations.length > 0) {
    console.error(
      JSON.stringify(
        {
          status: "error",
          violationCount: violations.length,
          violations,
        },
        null,
        2,
      ),
    );
    process.exitCode = 1;
    return;
  }

  console.log(
    JSON.stringify(
      {
        status: "ok",
        checkedDocs: requiredDocs.map(({ relativePath }) => relativePath),
        checkedApplicationFiles: applicationFiles.length,
        checkedWorkflow: path.relative(root, workflowPath),
        checkedSonarProject: path.relative(root, sonarProjectPath),
      },
      null,
      2,
    ),
  );
}

main().catch((error) => {
  console.error(
    JSON.stringify(
      {
        status: "error",
        message: error instanceof Error ? error.message : String(error),
      },
      null,
      2,
    ),
  );
  process.exitCode = 1;
});
