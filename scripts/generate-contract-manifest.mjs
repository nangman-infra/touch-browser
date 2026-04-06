import { mkdir, readdir, writeFile } from "node:fs/promises";
import path from "node:path";

async function collectSchemaFiles(directory, prefix = "") {
  const entries = await readdir(directory, { withFileTypes: true });
  const files = [];

  for (const entry of entries) {
    const entryPath = path.join(directory, entry.name);
    const nextPrefix = path.join(prefix, entry.name);

    if (entry.isDirectory()) {
      files.push(...(await collectSchemaFiles(entryPath, nextPrefix)));
      continue;
    }

    if (entry.isFile() && entry.name.endsWith(".schema.json")) {
      files.push(nextPrefix);
    }
  }

  return files.sort();
}

async function main() {
  const root = process.cwd();
  const schemaDir = path.join(root, "contracts", "schemas");
  const generatedDir = path.join(root, "contracts", "generated");
  const manifestPath = path.join(generatedDir, "manifest.json");
  const schemas = await collectSchemaFiles(schemaDir);

  await mkdir(generatedDir, { recursive: true });
  await writeFile(
    manifestPath,
    JSON.stringify(
      {
        generatedAt: new Date().toISOString(),
        schemas,
      },
      null,
      2,
    ),
  );

  console.log(
    JSON.stringify(
      {
        status: "ok",
        manifest: path.relative(root, manifestPath),
        schemaCount: schemas.length,
      },
      null,
      2,
    ),
  );
}

try {
  await main();
} catch (error) {
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
}
