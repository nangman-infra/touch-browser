import path from "node:path";

import type { ValidateFunction } from "ajv";
import Ajv2020Module from "ajv/dist/2020.js";

import { listFilesRecursive, readJsonFile } from "../support/json.js";
import { fixtureSchemaPath, fixturesRoot, repoRoot } from "../support/paths.js";

export type FixtureCase = {
  readonly id: string;
  readonly title: string;
  readonly category:
    | "static-docs"
    | "navigation"
    | "citation-heavy"
    | "hostile";
  readonly sourceUri: string;
  readonly htmlPath: string;
  readonly expectedSnapshotPath: string;
  readonly expectedEvidencePath: string;
  readonly risk: "low" | "medium" | "hostile";
  readonly expectations: {
    readonly mustContainTexts: readonly string[];
    readonly expectedCitationUrl?: string;
    readonly expectedKinds?: readonly string[];
    readonly claimChecks: readonly {
      readonly id: string;
      readonly statement: string;
      readonly expectedStatus: "supported" | "unsupported";
    }[];
    readonly hostileSignals?: readonly string[];
  };
};

export async function loadFixtureValidator(): Promise<
  ValidateFunction<FixtureCase>
> {
  type SchemaCompiler = <T>(schema: object) => ValidateFunction<T>;
  const Ajv2020 = Ajv2020Module as unknown as {
    new (options: {
      allErrors: boolean;
      strict: boolean;
      allowUnionTypes: boolean;
    }): { compile: SchemaCompiler };
  };
  const ajv = new Ajv2020({
    allErrors: true,
    strict: true,
    allowUnionTypes: true,
  });
  const schema = await readJsonFile<object>(fixtureSchemaPath);
  const validate = ajv.compile<FixtureCase>(schema);

  return validate;
}

export async function listFixtureMetadataFiles(): Promise<string[]> {
  return listFilesRecursive(
    fixturesRoot,
    (filename) => filename === "fixture.json",
  );
}

export async function loadFixtures(): Promise<FixtureCase[]> {
  const validate = await loadFixtureValidator();
  const metadataFiles = await listFixtureMetadataFiles();
  const fixtures: FixtureCase[] = [];

  for (const filePath of metadataFiles) {
    const fixture = await readJsonFile<FixtureCase>(filePath);
    const isValid = validate(fixture);

    if (!isValid) {
      throw new Error(
        `Invalid fixture metadata in ${path.relative(repoRoot, filePath)}: ${JSON.stringify(
          validate.errors,
          null,
          2,
        )}`,
      );
    }

    fixtures.push(fixture);
  }

  return fixtures;
}

export function resolveFixtureHtmlPath(fixture: FixtureCase): string {
  return path.join(repoRoot, fixture.htmlPath);
}

export function resolveFixtureSnapshotPath(fixture: FixtureCase): string {
  return path.join(repoRoot, fixture.expectedSnapshotPath);
}

export function resolveFixtureEvidencePath(fixture: FixtureCase): string {
  return path.join(repoRoot, fixture.expectedEvidencePath);
}
