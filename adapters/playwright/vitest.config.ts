import path from "node:path";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";

const rootDir = path.dirname(fileURLToPath(import.meta.url));
const coverageDirectory = path.resolve(
  rootDir,
  "../../sonar-reports/adapters-playwright",
);

export default defineConfig({
  root: rootDir,
  test: {
    include: ["src/**/*.test.ts"],
    passWithNoTests: true,
    coverage: {
      provider: "v8",
      reporter: ["lcovonly"],
      reportsDirectory: coverageDirectory,
      include: ["src/**/*.ts"],
      exclude: ["src/**/*.test.ts"],
    },
  },
});
