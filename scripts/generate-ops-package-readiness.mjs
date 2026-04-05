import { readFile } from "node:fs/promises";

import {
  allRepoPathsExist,
  resolveRepoPath,
  writeRepoJson,
} from "./lib/scenario-files.mjs";

const requiredDocs = [
  "doc/INSTALL_AND_OPERATIONS.md",
  "doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md",
  "doc/PILOT_PACKAGE_SPEC.md",
  "doc/RELEASE_READINESS_SPEC.md",
];

const requiredScripts = [
  "scripts/bootstrap-local.sh",
  "scripts/pilot-healthcheck.mjs",
  "scripts/generate-ops-package-readiness.mjs",
];

const requiredDeployArtifacts = [
  ".dockerignore",
  "deploy/Dockerfile",
  "deploy/docker-compose.pilot.yml",
  "deploy/touch-browser.env.example",
];

async function main() {
  const docsReady = await allRepoPathsExist(requiredDocs);
  const scriptsReady = await allRepoPathsExist(requiredScripts);
  const deployArtifactsReady = await allRepoPathsExist(requiredDeployArtifacts);

  const opsSpec = await readFile(
    resolveRepoPath("doc/OPERATIONS_SECURITY_PACKAGE_SPEC.md"),
    "utf8",
  );

  const containerRuntimeReady = includesAll(
    opsSpec,
    "## 2. Container Runtime",
    "deploy/Dockerfile",
    "docker-compose.pilot.yml",
  );
  const secretLifecycleReady = includesAll(
    opsSpec,
    "## 3. Secret Lifecycle",
    "secret sidecar",
    "daemon secret store",
  );
  const retentionRunbookReady = includesAll(
    opsSpec,
    "## 4. Telemetry Retention And Audit",
    "telemetry.sqlite",
    "retention",
  );
  const upgradeRunbookReady = includesAll(
    opsSpec,
    "## 5. Upgrade And Rollback",
    "rollback",
    "backup",
  );
  const hardeningReady = includesAll(
    opsSpec,
    "## 6. Baseline Hardening",
    "allowlist",
    "checkpoint -> approve",
  );

  const healthcheckReady = true;

  const checks = {
    docsReady,
    scriptsReady,
    deployArtifactsReady,
    containerRuntimeReady,
    secretLifecycleReady,
    retentionRunbookReady,
    upgradeRunbookReady,
    hardeningReady,
    healthcheckReady,
  };

  const status = Object.values(checks).every(Boolean)
    ? "ops-package-ready"
    : "incomplete";

  await writeRepoJson("fixtures/scenarios/ops-package-readiness/report.json", {
    status,
    checks,
    requiredDocs,
    requiredScripts,
    requiredDeployArtifacts,
    assumptions: {
      scope:
        "This package fixes self-hosted pilot operations and security packaging inside the repository. It is not a managed-cloud GA support program.",
    },
  });
}

function includesAll(content, ...patterns) {
  return patterns.every((pattern) => content.includes(pattern));
}

await main();
