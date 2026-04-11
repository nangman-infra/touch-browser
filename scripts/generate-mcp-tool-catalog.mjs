import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";

function requiredConst(properties, field) {
  const value = properties?.[field]?.const;
  if (value === undefined) {
    throw new Error(`MCP tool catalog schema is missing \`${field}\` const.`);
  }
  return value;
}

function extractToolCatalog(schema) {
  if (schema.$id !== "mcp-tool-catalog.schema.json") {
    throw new Error("Unexpected MCP tool catalog schema id.");
  }
  if (schema.type !== "array" || !Array.isArray(schema.prefixItems)) {
    throw new Error(
      "MCP tool catalog schema must be an array with prefixItems.",
    );
  }

  return schema.prefixItems.map((item) => {
    const properties = item?.properties;
    return {
      name: requiredConst(properties, "name"),
      title: requiredConst(properties, "title"),
      description: requiredConst(properties, "description"),
      inputSchema: requiredConst(properties, "inputSchema"),
    };
  });
}

export async function generateMcpToolCatalog(root) {
  const schemaPath = path.join(
    root,
    "contracts",
    "schemas",
    "mcp-tool-catalog.schema.json",
  );
  const generatedDir = path.join(root, "contracts", "generated");
  const generatedJsonPath = path.join(generatedDir, "mcp-tool-catalog.json");
  const generatedModulePath = path.join(generatedDir, "mcp-tool-catalog.mjs");
  const schema = JSON.parse(await readFile(schemaPath, "utf8"));
  const toolCatalog = extractToolCatalog(schema);

  await mkdir(generatedDir, { recursive: true });
  await writeFile(
    `${generatedJsonPath}`,
    `${JSON.stringify(toolCatalog, null, 2)}\n`,
  );
  await writeFile(
    generatedModulePath,
    `export const toolCatalog = ${JSON.stringify(toolCatalog, null, 2)};\n`,
  );

  return {
    schema: path.relative(root, schemaPath),
    generatedJson: path.relative(root, generatedJsonPath),
    generatedModule: path.relative(root, generatedModulePath),
    toolCount: toolCatalog.length,
  };
}

async function main() {
  const result = await generateMcpToolCatalog(process.cwd());
  console.log(JSON.stringify({ status: "ok", ...result }, null, 2));
}

if (import.meta.url === new URL(process.argv[1], "file:").href) {
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
}
