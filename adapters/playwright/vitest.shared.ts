import path from "node:path";
import { fileURLToPath } from "node:url";

const adapterRootDir = path.dirname(fileURLToPath(import.meta.url));

export const playwrightAdapterRootDir = adapterRootDir;
export const playwrightAdapterCoverageDirectory = path.resolve(
  adapterRootDir,
  "../../sonar-reports/adapters-playwright",
);
export const playwrightAdapterCoverageConfig = {
  provider: "v8" as const,
  reporter: ["lcovonly"] as string[],
  reportsDirectory: playwrightAdapterCoverageDirectory,
  include: ["src/**/*.ts"] as string[],
  exclude: ["tests/**/*.test.ts"] as string[],
};
