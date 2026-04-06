import { defineConfig } from "vitest/config";

import { evalsRootDir } from "./vitest.shared.js";

export default defineConfig({
  root: evalsRootDir,
  test: {
    include: ["tests/runtime/proof/**/*.test.ts"],
    passWithNoTests: true,
  },
});
