import {
  mkdir,
  mkdtemp,
  readFile,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { type BrowserContext, type Page, chromium } from "playwright";

import {
  ignoreCleanupFailure,
  ignoreNavigationSettleFailure,
  readProbeFallback,
} from "./error-tolerance.js";
import {
  hasSearchIdentityMarker,
  installSearchIdentity,
  searchIdentityPersistentOptions,
  writeSearchIdentityMarker,
} from "./search-identity.js";
import { describeUnknownValue, normalizeWhitespace } from "./shared.js";
import {
  type BrowserPageState,
  type BrowserSource,
  CONTEXT_LOCK_RETRY_MS,
  CONTEXT_LOCK_STALE_MS,
  CONTEXT_LOCK_TIMEOUT_MS,
  MAX_CAPTURED_LINKS,
  PAGE_ACTION_TIMEOUT_MS,
  PAGE_NAVIGATION_TIMEOUT_MS,
  SEARCH_PROFILE_POST_LOAD_IDLE_MS,
  SEARCH_PROFILE_POST_LOAD_WAIT_MS,
} from "./types.js";

export function browserSource(
  url: string | undefined,
  html: string | undefined,
  headless: boolean,
  contextDir: string | undefined,
  profileDir: string | undefined,
  searchIdentity: boolean,
): BrowserSource {
  return {
    url,
    html,
    contextDir,
    profileDir,
    headless,
    searchIdentity,
  };
}

export async function withPage<T>(
  source: BrowserSource,
  run: (page: Page) => Promise<T>,
): Promise<T> {
  const persistentDir = source.contextDir ?? source.profileDir;

  if (persistentDir) {
    return withContextDirLock(persistentDir, async () => {
      const effectiveSource = {
        ...source,
        searchIdentity:
          source.searchIdentity ||
          (source.contextDir
            ? await hasSearchIdentityMarker(source.contextDir)
            : false),
      } satisfies BrowserSource;
      const context = await launchPersistentBrowserContext(
        effectiveSource,
        persistentDir,
      );

      try {
        const page = context.pages()[0] ?? (await context.newPage());
        applyPageTimeouts(page);
        const shouldLoad =
          !!effectiveSource.html ||
          page.url() === "about:blank" ||
          (effectiveSource.url !== undefined &&
            !sameResolvedUrl(page.url(), effectiveSource.url));
        if (shouldLoad) {
          await loadPageSource(page, effectiveSource);
        }
        return await run(page);
      } finally {
        await closeContextQuietly(context);
      }
    });
  }

  if (source.searchIdentity) {
    const tempContextDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-search-profile-"),
    );
    try {
      const context = await launchPersistentBrowserContext(
        source,
        tempContextDir,
      );
      try {
        const page = context.pages()[0] ?? (await context.newPage());
        applyPageTimeouts(page);
        await loadPageSource(page, source);
        return await run(page);
      } finally {
        await closeContextQuietly(context);
      }
    } finally {
      await ignoreCleanupFailure(
        rm(tempContextDir, { recursive: true, force: true }),
        "withPage temp search profile cleanup",
      );
    }
  }

  const browser = await launchBrowser(source);

  try {
    const context = await browser.newContext({
      viewport: { width: 1600, height: 1200 },
      screen: { width: 1600, height: 1200 },
    });
    const page = await context.newPage();
    applyPageTimeouts(page);

    try {
      await loadPageSource(page, source);
      return await run(page);
    } finally {
      await page.close();
      await context.close();
    }
  } finally {
    await browser.close();
  }
}

export async function capturePageState(page: Page): Promise<BrowserPageState> {
  let lastError: unknown;

  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const title = await page.title();
      const visibleText = normalizeWhitespace(
        await readProbeFallback(
          page.locator("body").innerText(),
          "",
          "capturePageState body text",
        ),
      );
      const linkCount = await page.locator("a").count();
      const buttonCount = await page.locator("button").count();
      const inputCount = await page.locator("input").count();
      const links = await readLinks(page, linkCount);
      const html = await page.content();

      return {
        finalUrl: page.url(),
        title,
        visibleText,
        html,
        htmlLength: html.length,
        linkCount,
        buttonCount,
        inputCount,
        links,
      };
    } catch (error) {
      lastError = error;
      const message = error instanceof Error ? error.message : String(error);
      if (
        !message.includes("Execution context was destroyed") &&
        !message.includes("most likely because of a navigation")
      ) {
        throw error;
      }

      await ignoreNavigationSettleFailure(
        page.waitForLoadState("domcontentloaded", { timeout: 1000 }),
        "capturePageState domcontentloaded retry",
      );
      await ignoreNavigationSettleFailure(
        page.waitForLoadState("networkidle", { timeout: 1000 }),
        "capturePageState networkidle retry",
      );
      await ignoreNavigationSettleFailure(
        page.waitForTimeout(150),
        "capturePageState retry backoff",
      );
    }
  }

  throw lastError instanceof Error
    ? lastError
    : new Error(
        describeUnknownValue(lastError, "Unknown browser page state error"),
      );
}

async function readLinks(
  page: Page,
  linkCount: number,
): Promise<Array<{ text: string; href: string | null }>> {
  const links = [];

  for (
    let index = 0;
    index < Math.min(linkCount, MAX_CAPTURED_LINKS);
    index += 1
  ) {
    const locator = page.locator("a").nth(index);
    const [text, href] = await Promise.all([
      readProbeFallback(locator.textContent(), "", `readLinks text ${index}`),
      readProbeFallback(
        locator.getAttribute("href"),
        null,
        `readLinks href ${index}`,
      ),
    ]);
    links.push({
      text: normalizeWhitespace(text ?? ""),
      href,
    });
  }

  return links;
}

function applyPageTimeouts(page: Page): void {
  page.setDefaultNavigationTimeout(PAGE_NAVIGATION_TIMEOUT_MS);
  page.setDefaultTimeout(PAGE_ACTION_TIMEOUT_MS);
}

async function launchPersistentBrowserContext(
  source: BrowserSource,
  contextDir: string,
): Promise<BrowserContext> {
  const baseOptions = {
    headless: source.headless,
    viewport: { width: 1600, height: 1200 },
    screen: { width: 1600, height: 1200 },
  };
  const contextOptions = {
    ...baseOptions,
    ...(await searchIdentityPersistentOptions(source)),
  };
  const context = await chromium.launchPersistentContext(
    contextDir,
    contextOptions,
  );

  if (source.searchIdentity) {
    if (source.contextDir) {
      await writeSearchIdentityMarker(contextDir);
    }
    await installSearchIdentity(context, source);
  }
  return context;
}

async function launchBrowser(source: BrowserSource) {
  return chromium.launch({ headless: source.headless });
}

async function withContextDirLock<T>(
  contextDir: string,
  run: () => Promise<T>,
): Promise<T> {
  const release = await acquireContextDirLock(contextDir);

  try {
    return await run();
  } finally {
    await release();
  }
}

async function acquireContextDirLock(
  contextDir: string,
): Promise<() => Promise<void>> {
  const lockPath = `${contextDir}.touch-browser-lock`;
  const ownerPath = path.join(lockPath, "owner.json");
  const startedAt = Date.now();
  const owner = JSON.stringify(
    {
      pid: process.pid,
      startedAt: new Date(startedAt).toISOString(),
    },
    null,
    2,
  );
  await mkdir(path.dirname(lockPath), { recursive: true });

  while (Date.now() - startedAt < CONTEXT_LOCK_TIMEOUT_MS) {
    try {
      await mkdir(lockPath);
      await writeFile(ownerPath, owner, "utf8");
      return async () => {
        await rm(lockPath, { recursive: true, force: true });
      };
    } catch (error) {
      if (!isAlreadyExistsError(error)) {
        throw error;
      }

      await maybeRemoveStaleContextLock(lockPath, ownerPath);
      await delay(CONTEXT_LOCK_RETRY_MS);
    }
  }

  const staleHint = await readLockOwnerHint(ownerPath);
  throw new Error(
    `Persistent browser session is busy for \`${contextDir}\`. ${staleHint}Retry after the active browser action finishes.`,
  );
}

async function maybeRemoveStaleContextLock(
  lockPath: string,
  ownerPath: string,
): Promise<void> {
  try {
    const ownerStat = await stat(ownerPath);
    if (Date.now() - ownerStat.mtimeMs > CONTEXT_LOCK_STALE_MS) {
      await rm(lockPath, { recursive: true, force: true });
      return;
    }
  } catch {
    try {
      const lockStat = await stat(lockPath);
      if (Date.now() - lockStat.mtimeMs > CONTEXT_LOCK_STALE_MS) {
        await rm(lockPath, { recursive: true, force: true });
      }
    } catch {
      // A competing process may have released the lock while we were checking.
    }
  }
}

async function readLockOwnerHint(ownerPath: string): Promise<string> {
  try {
    const owner = JSON.parse(await readFile(ownerPath, "utf8")) as {
      pid?: number;
      startedAt?: string;
    };
    const parts = [];
    if (typeof owner.pid === "number") {
      parts.push(`Owner pid ${owner.pid}.`);
    }
    if (typeof owner.startedAt === "string") {
      parts.push(`Started ${owner.startedAt}.`);
    }
    if (parts.length > 0) {
      return `${parts.join(" ")} `;
    }
  } catch {
    // Ignore unreadable or missing metadata.
  }

  return "";
}

function isAlreadyExistsError(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: string }).code === "EEXIST"
  );
}

async function closeContextQuietly(context: BrowserContext): Promise<void> {
  await ignoreCleanupFailure(context.close(), "closeContextQuietly context");
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    setTimeout(resolve, ms);
  });
}

async function loadPageSource(
  page: Page,
  source: BrowserSource,
): Promise<void> {
  if (source.html) {
    if (source.searchIdentity) {
      const dataUrl = `data:text/html;charset=utf-8,${encodeURIComponent(source.html)}`;
      await page.goto(dataUrl, {
        waitUntil: "domcontentloaded",
        timeout: PAGE_NAVIGATION_TIMEOUT_MS,
      });
    } else {
      await page.setContent(source.html, {
        waitUntil: "domcontentloaded",
        timeout: PAGE_NAVIGATION_TIMEOUT_MS,
      });
    }
  } else if (source.url) {
    await page.goto(source.url, {
      waitUntil: "domcontentloaded",
      timeout: PAGE_NAVIGATION_TIMEOUT_MS,
    });
    if (source.searchIdentity) {
      await ignoreNavigationSettleFailure(
        page.waitForLoadState("networkidle", {
          timeout: SEARCH_PROFILE_POST_LOAD_IDLE_MS,
        }),
        "loadPageSource search profile networkidle",
      );
      await ignoreNavigationSettleFailure(
        page.waitForTimeout(SEARCH_PROFILE_POST_LOAD_WAIT_MS),
        "loadPageSource search profile settle wait",
      );
    }
  }
}

function sameResolvedUrl(left: string, right: string): boolean {
  try {
    return new URL(left).href === new URL(right).href;
  } catch {
    return left === right;
  }
}
