import { access } from "node:fs/promises";
import path from "node:path";

const requiredPaths = [
  "contracts",
  "contracts/schemas",
  "contracts/generated",
  "contracts/generated/ts",
  "contracts/generated/rust",
];

async function main() {
  const root = process.cwd();

  for (const relativePath of requiredPaths) {
    const absolutePath = path.join(root, relativePath);
    await access(absolutePath);
  }

  console.log(
    JSON.stringify(
      {
        status: "ok",
        checked: requiredPaths,
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
