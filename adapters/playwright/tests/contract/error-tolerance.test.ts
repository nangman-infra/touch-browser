import { describe, expect, it, vi } from "vitest";

import {
  ignoreCleanupFailure,
  ignoreNavigationSettleFailure,
  ignoreOptionalActionFailure,
  readProbeFallback,
} from "../../src/error-tolerance.js";

describe("playwright adapter error tolerance helpers", () => {
  it("returns explicit fallback values for read probes", async () => {
    await expect(
      readProbeFallback(Promise.reject(new Error("probe failed")), "", "probe"),
    ).resolves.toBe("");
    await expect(
      readProbeFallback(
        Promise.reject(new Error("probe failed")),
        false,
        "visibility",
      ),
    ).resolves.toBe(false);
  });

  it("swallows cleanup and settle failures without changing the call path", async () => {
    await expect(
      ignoreCleanupFailure(
        Promise.reject(new Error("cleanup failed")),
        "cleanup",
      ),
    ).resolves.toBeUndefined();
    await expect(
      ignoreNavigationSettleFailure(
        Promise.reject(new Error("settle failed")),
        "settle",
      ),
    ).resolves.toBeUndefined();
    await expect(
      ignoreOptionalActionFailure(
        Promise.reject(new Error("optional action failed")),
        "optional-action",
      ),
    ).resolves.toBeUndefined();
  });

  it("emits classified debug output when the debug env is enabled", async () => {
    const previous = process.env.TOUCH_BROWSER_DEBUG_PLAYWRIGHT_ERRORS;
    process.env.TOUCH_BROWSER_DEBUG_PLAYWRIGHT_ERRORS = "1";
    const stderrSpy = vi
      .spyOn(process.stderr, "write")
      .mockImplementation(() => true);

    try {
      await ignoreCleanupFailure(
        Promise.reject(new Error("cleanup failed")),
        "bridge cleanup",
      );
      expect(stderrSpy).toHaveBeenCalled();
      expect(stderrSpy.mock.calls[0]?.[0]).toContain(
        "[touch-browser][playwright:cleanup] bridge cleanup: cleanup failed",
      );
    } finally {
      stderrSpy.mockRestore();
      if (previous === undefined) {
        process.env.TOUCH_BROWSER_DEBUG_PLAYWRIGHT_ERRORS = undefined;
      } else {
        process.env.TOUCH_BROWSER_DEBUG_PLAYWRIGHT_ERRORS = previous;
      }
    }
  });
});
