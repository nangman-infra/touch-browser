import { mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "..");
const fixturesRoot = path.join(repoRoot, "fixtures", "research");
const outputDir = path.join(
  repoRoot,
  "fixtures",
  "scenarios",
  "citation-metrics",
);

async function main() {
  const metadataPaths = await listFixtureMetadataPaths(fixturesRoot);
  const reports = [];

  for (const metadataPath of metadataPaths) {
    const fixture = JSON.parse(await readFile(metadataPath, "utf8"));
    const evidence = JSON.parse(
      await readFile(path.join(repoRoot, fixture.expectedEvidencePath), "utf8"),
    );
    const snapshot = JSON.parse(
      await readFile(path.join(repoRoot, fixture.expectedSnapshotPath), "utf8"),
    );
    reports.push(analyzeFixture(fixture, evidence, snapshot));
  }

  const report = {
    fixtureCount: reports.length,
    averageClassificationPrecision: averageOf(
      reports,
      "classificationPrecision",
    ),
    averageClassificationRecall: averageOf(reports, "classificationRecall"),
    averageCitationPrecision: averageOf(reports, "citationPrecision"),
    averageCitationRecall: averageOf(reports, "citationRecall"),
    averageUnsupportedPrecision: averageOf(reports, "unsupportedPrecision"),
    averageUnsupportedRecall: averageOf(reports, "unsupportedRecall"),
    averageSupportReferencePrecision: averageOf(
      reports,
      "supportReferencePrecision",
    ),
    sourceAlignmentRate: roundTo(
      reports.filter((fixtureReport) => fixtureReport.sourceAligned).length /
        Math.max(reports.length, 1),
      2,
    ),
    fixtures: reports,
  };

  await mkdir(outputDir, { recursive: true });
  await writeFile(
    path.join(outputDir, "report.json"),
    `${JSON.stringify(report, null, 2)}\n`,
  );
}

function analyzeFixture(fixture, evidence, snapshot) {
  const expectedSupportedIds = new Set(
    fixture.expectations.claimChecks
      .filter((claim) => claim.expectedStatus === "supported")
      .map((claim) => claim.id),
  );
  const expectedUnsupportedIds = new Set(
    fixture.expectations.claimChecks
      .filter((claim) => claim.expectedStatus === "unsupported")
      .map((claim) => claim.id),
  );
  const actualSupported = evidence.evidenceSupportedClaims ?? [];
  const actualUnsupported = [
    ...(evidence.contradictedClaims ?? []),
    ...(evidence.insufficientEvidenceClaims ?? []),
    ...(evidence.needsMoreBrowsingClaims ?? []),
  ];
  const actualSupportedIds = new Set(
    actualSupported.map((claim) => claim.claimId),
  );
  const actualUnsupportedIds = new Set(
    actualUnsupported.map((claim) => claim.claimId),
  );
  const snapshotBlockIds = new Set(snapshot.blocks.map((block) => block.id));

  const correctSupportedCount = actualSupported.filter((claim) =>
    expectedSupportedIds.has(claim.claimId),
  ).length;
  const correctUnsupportedCount = actualUnsupported.filter((claim) =>
    expectedUnsupportedIds.has(claim.claimId),
  ).length;

  const citationValidSupportedClaims = actualSupported.filter((claim) =>
    hasValidCitation(claim, fixture, evidence, snapshotBlockIds),
  );
  const correctAndCitationValidCount = citationValidSupportedClaims.filter(
    (claim) => expectedSupportedIds.has(claim.claimId),
  ).length;
  const supportReferenceCount = actualSupported.reduce(
    (sum, claim) => sum + (claim.support?.length ?? 0),
    0,
  );
  const validSupportReferenceCount = actualSupported.reduce(
    (sum, claim) =>
      sum +
      (claim.support ?? []).filter((blockId) => snapshotBlockIds.has(blockId))
        .length,
    0,
  );

  return {
    id: fixture.id,
    category: fixture.category,
    expectedSupportedCount: expectedSupportedIds.size,
    expectedUnsupportedCount: expectedUnsupportedIds.size,
    actualSupportedCount: actualSupported.length,
    actualUnsupportedCount: actualUnsupported.length,
    classificationPrecision: ratio(
      correctSupportedCount,
      actualSupported.length,
    ),
    classificationRecall: ratio(
      correctSupportedCount,
      expectedSupportedIds.size,
    ),
    unsupportedPrecision: ratio(
      correctUnsupportedCount,
      actualUnsupported.length,
    ),
    unsupportedRecall: ratio(
      correctUnsupportedCount,
      expectedUnsupportedIds.size,
    ),
    citationPrecision: ratio(
      correctAndCitationValidCount,
      actualSupported.length,
    ),
    citationRecall: ratio(
      correctAndCitationValidCount,
      expectedSupportedIds.size,
    ),
    supportReferencePrecision: ratio(
      validSupportReferenceCount,
      supportReferenceCount,
    ),
    sourceAligned:
      evidence.source?.sourceUrl === fixture.sourceUri &&
      evidence.source?.sourceRisk === fixture.risk,
  };
}

function hasValidCitation(claim, fixture, evidence, snapshotBlockIds) {
  const citation = claim.citation;
  if (!citation || typeof citation !== "object") {
    return false;
  }

  return (
    citation.url === fixture.sourceUri &&
    citation.sourceType === evidence.source?.sourceType &&
    citation.sourceRisk === fixture.risk &&
    typeof citation.retrievedAt === "string" &&
    citation.retrievedAt.length > 0 &&
    Array.isArray(claim.support) &&
    claim.support.length > 0 &&
    claim.support.every((blockId) => snapshotBlockIds.has(blockId))
  );
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

function averageOf(reports, key) {
  return roundTo(
    reports.reduce((sum, report) => sum + report[key], 0) /
      Math.max(reports.length, 1),
    2,
  );
}

function ratio(numerator, denominator) {
  if (denominator <= 0) {
    return 1;
  }

  return roundTo(numerator / denominator, 2);
}

function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}

await main();
