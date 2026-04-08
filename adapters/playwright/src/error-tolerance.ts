import { describeUnknownValue } from "./shared.js";

type IgnoredPlaywrightErrorKind =
  | "probe"
  | "cleanup"
  | "navigation-settle"
  | "optional-action";

export async function readProbeFallback<T>(
  operation: Promise<T>,
  fallbackValue: T,
  context: string,
): Promise<T> {
  try {
    return await operation;
  } catch (error) {
    reportIgnoredPlaywrightError("probe", context, error);
    return fallbackValue;
  }
}

export async function ignoreCleanupFailure(
  operation: Promise<unknown>,
  context: string,
): Promise<void> {
  await ignorePlaywrightFailure(operation, "cleanup", context);
}

export async function ignoreNavigationSettleFailure(
  operation: Promise<unknown>,
  context: string,
): Promise<void> {
  await ignorePlaywrightFailure(operation, "navigation-settle", context);
}

export async function ignoreOptionalActionFailure(
  operation: Promise<unknown>,
  context: string,
): Promise<void> {
  await ignorePlaywrightFailure(operation, "optional-action", context);
}

async function ignorePlaywrightFailure(
  operation: Promise<unknown>,
  kind: IgnoredPlaywrightErrorKind,
  context: string,
): Promise<void> {
  try {
    await operation;
  } catch (error) {
    reportIgnoredPlaywrightError(kind, context, error);
  }
}

function reportIgnoredPlaywrightError(
  kind: IgnoredPlaywrightErrorKind,
  context: string,
  error: unknown,
): void {
  if (!playwrightFallbackDebugEnabled()) {
    return;
  }

  const message = describeUnknownValue(error, "Unknown Playwright fallback");
  process.stderr.write(
    `[touch-browser][playwright:${kind}] ${context}: ${message}\n`,
  );
}

function playwrightFallbackDebugEnabled(): boolean {
  return (
    truthyEnv(process.env.TOUCH_BROWSER_DEBUG_PLAYWRIGHT_ERRORS) ||
    truthyEnv(process.env.TOUCH_BROWSER_DEBUG_PLAYWRIGHT)
  );
}

function truthyEnv(value: string | undefined): boolean {
  if (!value) {
    return false;
  }

  return !["0", "false", "no", "off"].includes(value.trim().toLowerCase());
}
