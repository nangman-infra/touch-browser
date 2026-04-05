import { mkdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { repoRoot } from "./lib/live-sample-server.mjs";

const outputDir = path.join(
  repoRoot,
  "fixtures",
  "scenarios",
  "action-surface-regression",
);

const coreSurface = [
  "open",
  "extract",
  "policy",
  "compact-view",
  "follow",
  "paginate",
  "expand",
  "session-synthesize",
];

const expandedSurface = [
  ...coreSurface,
  "get-html",
  "get-visible-text",
  "list-links",
  "click-by-text",
  "click-by-href",
  "search-page",
  "summarize-page",
  "get-title",
];

const tasks = [
  { id: "open-doc", coreCandidates: 1, expandedCandidates: 4 },
  { id: "extract-claims", coreCandidates: 1, expandedCandidates: 3 },
  { id: "policy-check", coreCandidates: 1, expandedCandidates: 2 },
  { id: "navigate-follow", coreCandidates: 1, expandedCandidates: 3 },
  { id: "paginate", coreCandidates: 1, expandedCandidates: 2 },
  { id: "expand", coreCandidates: 1, expandedCandidates: 2 },
  { id: "synthesize-session", coreCandidates: 1, expandedCandidates: 4 },
];

async function main() {
  const taskReports = tasks.map((task) => ({
    id: task.id,
    coreCandidates: task.coreCandidates,
    expandedCandidates: task.expandedCandidates,
    coreWrongToolOpportunityRate: wrongToolRate(task.coreCandidates),
    expandedWrongToolOpportunityRate: wrongToolRate(task.expandedCandidates),
  }));

  const report = {
    coreSurfaceCount: coreSurface.length,
    expandedSurfaceCount: expandedSurface.length,
    averageCoreWrongToolOpportunityRate: averageOf(
      taskReports,
      "coreWrongToolOpportunityRate",
    ),
    averageExpandedWrongToolOpportunityRate: averageOf(
      taskReports,
      "expandedWrongToolOpportunityRate",
    ),
    wrongToolOpportunityReductionRate: roundTo(
      1 -
        averageOf(taskReports, "coreWrongToolOpportunityRate") /
          Math.max(
            averageOf(taskReports, "expandedWrongToolOpportunityRate"),
            0.01,
          ),
      2,
    ),
    tasks: taskReports,
  };

  await mkdir(outputDir, { recursive: true });
  await writeFile(
    path.join(outputDir, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}

function wrongToolRate(candidateCount) {
  if (candidateCount <= 1) {
    return 0;
  }

  return roundTo((candidateCount - 1) / candidateCount, 2);
}

function averageOf(items, key) {
  return roundTo(
    items.reduce((sum, item) => sum + item[key], 0) / Math.max(items.length, 1),
    2,
  );
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
