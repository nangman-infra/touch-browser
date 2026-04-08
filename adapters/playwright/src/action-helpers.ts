import type { Locator, Page } from "playwright";

import {
  ignoreNavigationSettleFailure,
  ignoreOptionalActionFailure,
  readProbeFallback,
} from "./error-tolerance.js";
import {
  ACTION_SETTLE_EXTRA_WAIT_MS,
  ACTION_SETTLE_IDLE_TIMEOUT_MS,
  ACTION_SETTLE_TIMEOUT_MS,
} from "./types.js";

export async function fillTargetLocator(
  page: Page,
  locator: Locator,
  value: string,
): Promise<void> {
  const tagName = await readProbeFallback(
    locator.evaluate((element) => element.tagName.toLowerCase()),
    "",
    "fillTargetLocator tagName",
  );
  const isContentEditable = await readProbeFallback(
    locator.evaluate((element) => element.hasAttribute("contenteditable")),
    false,
    "fillTargetLocator contenteditable",
  );

  if (tagName === "input" || tagName === "textarea") {
    await locator.fill(value);
  } else if (isContentEditable) {
    await locator.click();
    await page.keyboard.press("Meta+A").catch(async () => {
      await page.keyboard.press("Control+A");
    });
    await page.keyboard.type(value);
  } else {
    throw new Error("Target input does not support typing.");
  }

  await ignoreOptionalActionFailure(
    locator.dispatchEvent("input"),
    "fillTargetLocator input event",
  );
  await ignoreOptionalActionFailure(
    locator.dispatchEvent("change"),
    "fillTargetLocator change event",
  );
}

export async function submitTargetLocator(locator: Locator): Promise<void> {
  const tagName = await readProbeFallback(
    locator.evaluate((element) => element.tagName.toLowerCase()),
    "",
    "submitTargetLocator tagName",
  );

  if (tagName === "form") {
    await locator
      .evaluate((element) => {
        const form = element as HTMLFormElement;
        if (typeof form.requestSubmit === "function") {
          form.requestSubmit();
        } else {
          form.submit();
        }
      })
      .catch(async () => {
        await locator.press("Enter");
      });
    return;
  }

  await locator.click();
}

export function resolveSafeFollowUrl(
  currentUrl: string,
  href: string | undefined,
): string | undefined {
  if (!href) {
    return undefined;
  }

  try {
    const resolved = new URL(href, currentUrl);
    const current = new URL(currentUrl);
    if (
      resolved.origin === current.origin &&
      (href.startsWith("#") ||
        href.startsWith("/") ||
        href.startsWith("./") ||
        href.startsWith("../") ||
        resolved.pathname !== current.pathname ||
        resolved.search !== current.search ||
        resolved.hash !== current.hash)
    ) {
      return resolved.toString();
    }
  } catch {
    return undefined;
  }

  return undefined;
}

export function nextPaginationSelectors(): string[] {
  return [
    "a[rel='next']",
    "button[rel='next']",
    "[data-touch-browser-direction='next']",
    "[data-direction='next']",
    "button:has-text('Next')",
    "a:has-text('Next')",
    "button:has-text('More')",
    "a:has-text('More')",
    "button:has-text('Continue')",
    "a:has-text('Continue')",
  ];
}

export function prevPaginationSelectors(): string[] {
  return [
    "a[rel='prev']",
    "button[rel='prev']",
    "[data-touch-browser-direction='prev']",
    "[data-direction='prev']",
    "button:has-text('Previous')",
    "a:has-text('Previous')",
    "button:has-text('Back')",
    "a:has-text('Back')",
  ];
}

export async function settleAfterAction(page: Page): Promise<void> {
  await ignoreNavigationSettleFailure(
    page.waitForLoadState("domcontentloaded", {
      timeout: ACTION_SETTLE_TIMEOUT_MS,
    }),
    "settleAfterAction domcontentloaded",
  );
  await ignoreNavigationSettleFailure(
    page.waitForLoadState("networkidle", {
      timeout: ACTION_SETTLE_IDLE_TIMEOUT_MS,
    }),
    "settleAfterAction networkidle",
  );
  await ignoreNavigationSettleFailure(
    page.waitForTimeout(ACTION_SETTLE_EXTRA_WAIT_MS),
    "settleAfterAction extra wait",
  );
}
