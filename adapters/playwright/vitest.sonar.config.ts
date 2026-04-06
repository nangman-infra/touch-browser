import { defineConfig } from "vitest/config";

import {
  playwrightAdapterCoverageConfig,
  playwrightAdapterRootDir,
} from "./vitest.shared.js";

export default defineConfig({
  root: playwrightAdapterRootDir,
  test: {
    include: ["tests/contract/**/*.test.ts", "tests/browser-gate/**/*.test.ts"],
    passWithNoTests: true,
    coverage: playwrightAdapterCoverageConfig,
  },
});
