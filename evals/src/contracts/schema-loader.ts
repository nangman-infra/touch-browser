import { readFile } from "node:fs/promises";
import path from "node:path";

import type { AnySchemaObject, ValidateFunction } from "ajv";
import Ajv2020Module from "ajv/dist/2020.js";

import { listFilesRecursive, readJsonFile } from "../support/json.js";
import { contractsDir } from "../support/paths.js";

type AjvCompiler = {
  addSchema: (schema: AnySchemaObject, key?: string) => unknown;
  getSchema: (key: string) => ValidateFunction<unknown> | undefined;
};

type AjvConstructor = new (options: {
  allErrors: boolean;
  strict: boolean;
  allowUnionTypes: boolean;
}) => AjvCompiler;

export type SchemaRegistry = {
  readonly ajv: AjvCompiler;
  readonly schemas: ReadonlyMap<string, AnySchemaObject>;
};

export async function loadContractSchemas(): Promise<SchemaRegistry> {
  const schemaPaths = await listFilesRecursive(contractsDir, (filename) =>
    filename.endsWith(".schema.json"),
  );

  const schemas = new Map<string, AnySchemaObject>();
  const Ajv2020 = Ajv2020Module as unknown as AjvConstructor;
  const ajv = new Ajv2020({
    allErrors: true,
    strict: true,
    allowUnionTypes: true,
  });

  for (const schemaPath of schemaPaths) {
    const schema = await readJsonFile<AnySchemaObject>(schemaPath);
    const schemaId =
      typeof schema.$id === "string" && schema.$id.length > 0
        ? schema.$id
        : path.basename(schemaPath);

    ajv.addSchema(schema, schemaId);
    schemas.set(schemaId, schema);
  }

  return {
    ajv,
    schemas,
  };
}

export async function readSchemaSource(
  schemaPath: string,
): Promise<AnySchemaObject> {
  const raw = await readFile(schemaPath, "utf8");
  return JSON.parse(raw) as AnySchemaObject;
}

export function requireValidator(
  registry: SchemaRegistry,
  schemaId: string,
): ValidateFunction<unknown> {
  const validator = registry.ajv.getSchema(schemaId);

  if (!validator) {
    throw new Error(`Missing validator for schema: ${schemaId}`);
  }

  return validator;
}
