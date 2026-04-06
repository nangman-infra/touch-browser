import { defineConfig } from "vitest/config";

import { evalsRootDir } from "./vitest.shared.js";

export default defineConfig({
  root: evalsRootDir,
  test: {
    include: [
      "tests/contracts/**/*.test.ts",
      "tests/fixtures/**/*.test.ts",
      "tests/runtime/gate/**/*.test.ts",
    ],
    passWithNoTests: true,
  },
});
