import path from "node:path";
import { fileURLToPath } from "node:url";

const currentDir = path.dirname(fileURLToPath(import.meta.url));

export const repoRoot = path.resolve(currentDir, "../../..");
export const contractsDir = path.join(repoRoot, "contracts", "schemas");
export const contractsManifestPath = path.join(
  repoRoot,
  "contracts",
  "generated",
  "manifest.json",
);
export const fixturesRoot = path.join(repoRoot, "fixtures");
export const fixtureSchemaPath = path.join(
  fixturesRoot,
  "fixture-case.schema.json",
);
export const scenarioFixturesRoot = path.join(fixturesRoot, "scenarios");
