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
  captureFrameLinks,
  collectClosedShadowSnapshots,
  installDomInstrumentation,
} from "./dom-instrumentation.js";
import {
  ignoreCleanupFailure,
  ignoreNavigationSettleFailure,
  readProbeFallback,
} from "./error-tolerance.js";
import {
  hasSearchIdentityMarker as hasSearchIdentityProfileMarker,
  installSearchIdentity,
  searchIdentityPersistentOptions,
  writeSearchIdentityMarker,
} from "./search-identity.js";
import { describeUnknownValue, normalizeWhitespace } from "./shared.js";
import {
  type BrowserLoadDiagnostics,
  type BrowserPageState,
  type BrowserSource,
  CONTEXT_LOCK_RETRY_MS,
  CONTEXT_LOCK_STALE_MS,
  CONTEXT_LOCK_TIMEOUT_MS,
  GENERIC_POST_LOAD_PROBE_BUDGET_MS,
  GENERIC_POST_LOAD_PROBE_INTERVAL_MS,
  GENERIC_POST_LOAD_PROBE_MAX_MS,
  MAX_CAPTURED_LINKS,
  PAGE_ACTION_TIMEOUT_MS,
  PAGE_NAVIGATION_TIMEOUT_MS,
  SEARCH_MANUAL_RECOVERY_POLL_MS,
  SEARCH_MANUAL_RECOVERY_TIMEOUT_MS,
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
  manualRecovery: boolean,
): BrowserSource {
  return {
    url,
    html,
    contextDir,
    profileDir,
    headless,
    searchIdentity,
    manualRecovery,
  };
}

export async function withPage<T>(
  source: BrowserSource,
  run: (page: Page, loadDiagnostics: BrowserLoadDiagnostics) => Promise<T>,
): Promise<T> {
  const persistentDir = source.contextDir ?? source.profileDir;

  if (persistentDir) {
    return withContextDirLock(persistentDir, async () => {
      const effectiveSource = {
        ...source,
        searchIdentity:
          source.searchIdentity ||
          (source.contextDir
            ? await hasSearchIdentityProfileMarker(source.contextDir)
            : false),
      } satisfies BrowserSource;
      const context = await launchPersistentBrowserContext(
        effectiveSource,
        persistentDir,
      );

      try {
        const page = context.pages()[0] ?? (await context.newPage());
        applyPageTimeouts(page);
        let loadDiagnostics = idleLoadDiagnostics("reuse-existing-page");
        const shouldLoad =
          !!effectiveSource.html ||
          page.url() === "about:blank" ||
          (effectiveSource.url !== undefined &&
            !sameResolvedUrl(page.url(), effectiveSource.url));
        if (shouldLoad) {
          loadDiagnostics = await loadPageSource(page, effectiveSource);
        }
        await maybeAwaitManualSearchRecovery(page, effectiveSource);
        return await run(page, loadDiagnostics);
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
        const loadDiagnostics = await loadPageSource(page, source);
        await maybeAwaitManualSearchRecovery(page, source);
        return await run(page, loadDiagnostics);
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
      acceptDownloads: true,
    });
    await installDomInstrumentation(context);
    const page = await context.newPage();
    applyPageTimeouts(page);

    try {
      const loadDiagnostics = await loadPageSource(page, source);
      await maybeAwaitManualSearchRecovery(page, source);
      return await run(page, loadDiagnostics);
    } finally {
      await page.close();
      await context.close();
    }
  } finally {
    await browser.close();
  }
}

export async function capturePageState(
  page: Page,
  loadDiagnostics: BrowserLoadDiagnostics = idleLoadDiagnostics(
    "action-settle",
  ),
): Promise<BrowserPageState> {
  let lastError: unknown;

  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const title = await page.title();
      const baseVisibleText = normalizeWhitespace(
        await readProbeFallback(
          page.locator("body").innerText(),
          "",
          "capturePageState body text",
        ),
      );
      const baseLinkCount = await page.locator("a").count();
      const baseButtonCount = await page.locator("button").count();
      const baseInputCount = await page.locator("input").count();
      const baseLinks = await readLinks(page, baseLinkCount);
      const baseHtml = await page.content();
      const mainShadowSnapshots = await collectClosedShadowSnapshots(page);
      const childFrames = page
        .frames()
        .filter((frame) => frame !== page.mainFrame());
      const frameSnapshots = await Promise.all(
        childFrames.map(async (frame, index) => {
          const [
            html,
            visibleText,
            linkCount,
            buttonCount,
            inputCount,
            links,
            closedShadows,
          ] = await Promise.all([
            readProbeFallback(
              frame.content(),
              "",
              `capturePageState frame html ${index}`,
            ),
            readProbeFallback(
              frame.locator("body").innerText(),
              "",
              `capturePageState frame text ${index}`,
            ),
            readProbeFallback(
              frame.locator("a").count(),
              0,
              `capturePageState frame links ${index}`,
            ),
            readProbeFallback(
              frame.locator("button").count(),
              0,
              `capturePageState frame buttons ${index}`,
            ),
            readProbeFallback(
              frame.locator("input").count(),
              0,
              `capturePageState frame inputs ${index}`,
            ),
            captureFrameLinks(frame, MAX_CAPTURED_LINKS),
            collectClosedShadowSnapshots(frame),
          ]);
          return {
            url: frame.url(),
            html,
            visibleText: normalizeWhitespace(visibleText),
            linkCount,
            buttonCount,
            inputCount,
            links,
            closedShadows,
          };
        }),
      );
      const mainShadowSuffix = mainShadowSnapshots
        .map(
          (snapshot, index) =>
            `<section data-touch-browser-closed-shadow-root="${index}" data-touch-browser-host-path="${snapshot.hostPath}"><main>${snapshot.html}</main></section>`,
        )
        .join("");
      const frameSuffix = frameSnapshots
        .map((frame, index) => {
          const closedShadowSuffix = frame.closedShadows
            .map(
              (snapshot, shadowIndex) =>
                `<section data-touch-browser-closed-shadow-root="${index}-${shadowIndex}" data-touch-browser-host-path="${snapshot.hostPath}"><main>${snapshot.html}</main></section>`,
            )
            .join("");
          return `<section data-touch-browser-frame="${index}" data-touch-browser-frame-url="${frame.url}">${frame.html}${closedShadowSuffix}</section>`;
        })
        .join("");
      const visibleText = normalizeWhitespace(
        [
          baseVisibleText,
          ...mainShadowSnapshots.map((snapshot) => snapshot.text),
          ...frameSnapshots.flatMap((frame) => [
            frame.visibleText,
            ...frame.closedShadows.map((snapshot) => snapshot.text),
          ]),
        ].join(" "),
      );
      const linkCount =
        baseLinkCount +
        mainShadowSnapshots.reduce(
          (total, snapshot) => total + snapshot.linkCount,
          0,
        ) +
        frameSnapshots.reduce(
          (total, frame) =>
            total +
            frame.linkCount +
            frame.closedShadows.reduce(
              (shadowTotal, snapshot) => shadowTotal + snapshot.linkCount,
              0,
            ),
          0,
        );
      const buttonCount =
        baseButtonCount +
        mainShadowSnapshots.reduce(
          (total, snapshot) => total + snapshot.buttonCount,
          0,
        ) +
        frameSnapshots.reduce(
          (total, frame) =>
            total +
            frame.buttonCount +
            frame.closedShadows.reduce(
              (shadowTotal, snapshot) => shadowTotal + snapshot.buttonCount,
              0,
            ),
          0,
        );
      const inputCount =
        baseInputCount +
        mainShadowSnapshots.reduce(
          (total, snapshot) => total + snapshot.inputCount,
          0,
        ) +
        frameSnapshots.reduce(
          (total, frame) =>
            total +
            frame.inputCount +
            frame.closedShadows.reduce(
              (shadowTotal, snapshot) => shadowTotal + snapshot.inputCount,
              0,
            ),
          0,
        );
      const links = [
        ...baseLinks,
        ...mainShadowSnapshots.flatMap((snapshot) => snapshot.links),
        ...frameSnapshots.flatMap((frame) => [
          ...frame.links,
          ...frame.closedShadows.flatMap((snapshot) => snapshot.links),
        ]),
      ].slice(0, MAX_CAPTURED_LINKS);
      const html = `${baseHtml}${mainShadowSuffix}${frameSuffix}`;

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
        diagnostics: loadDiagnostics,
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
    acceptDownloads: true,
  };
  const contextOptions = {
    ...baseOptions,
    ...(await searchIdentityPersistentOptions(source)),
  };
  const context = await chromium.launchPersistentContext(
    contextDir,
    contextOptions,
  );
  await installDomInstrumentation(context);

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

type SearchChallengeSnapshot = {
  finalUrl: string;
  title: string;
  visibleText: string;
};

const JS_PLACEHOLDER_HINTS = [
  "enable javascript",
  "requires javascript",
  "javascript to run this app",
  "turn javascript on",
  "javascript is disabled",
  "you need to enable javascript",
] as const;

type PageQualityProbe = {
  ready: boolean;
  placeholderDetected: boolean;
  reason: string;
  score: number;
  mainBlockCount: number;
  shellBlockCount: number;
};

type PageQualityProbeStats = {
  bodyTextLength: number;
  mainBlockCount: number;
  mainTextLength: number;
  placeholderDetected: boolean;
  shellBlockCount: number;
  textLikeBlockCount: number;
};

async function maybeAwaitManualSearchRecovery(
  page: Page,
  source: BrowserSource,
): Promise<void> {
  if (!source.searchIdentity || !source.manualRecovery) {
    return;
  }

  let challenge = await readSearchChallengeSnapshot(page);
  if (!looksLikeSearchChallenge(challenge)) {
    return;
  }

  await ignoreNavigationSettleFailure(
    page.bringToFront(),
    "manual search recovery bringToFront",
  );

  const deadline = Date.now() + resolveSearchManualRecoveryTimeoutMs();
  while (Date.now() < deadline) {
    await ignoreNavigationSettleFailure(
      page.waitForLoadState("domcontentloaded", { timeout: 1_000 }),
      "manual search recovery domcontentloaded",
    );
    await ignoreNavigationSettleFailure(
      page.waitForLoadState("networkidle", { timeout: 1_000 }),
      "manual search recovery networkidle",
    );
    await ignoreNavigationSettleFailure(
      page.waitForTimeout(SEARCH_MANUAL_RECOVERY_POLL_MS),
      "manual search recovery poll wait",
    );

    challenge = await readSearchChallengeSnapshot(page);
    if (!looksLikeSearchChallenge(challenge)) {
      return;
    }
  }
}

async function readSearchChallengeSnapshot(
  page: Page,
): Promise<SearchChallengeSnapshot> {
  const [title, visibleText] = await Promise.all([
    readProbeFallback(page.title(), "", "search challenge page title"),
    readProbeFallback(
      page.locator("body").innerText(),
      "",
      "search challenge page body text",
    ),
  ]);

  return {
    finalUrl: page.url(),
    title: normalizeWhitespace(title).toLowerCase(),
    visibleText: normalizeWhitespace(visibleText).toLowerCase(),
  };
}

function looksLikeSearchChallenge(snapshot: SearchChallengeSnapshot): boolean {
  const combined = [
    snapshot.finalUrl.toLowerCase(),
    snapshot.title,
    snapshot.visibleText,
  ].join(" ");
  const signals = [
    "captcha",
    "recaptcha",
    "confirm you're not a robot",
    "i'm not a robot",
    "unusual traffic",
    "traffic verification",
    "verify you are human",
    "verify you're human",
    "robot check",
    "human checkpoint",
    "drag the slider",
    "security check",
    "비정상적인 트래픽",
    "로봇이 아닙니다",
  ];

  return (
    combined.includes("/sorry/") ||
    signals.some((signal) => combined.includes(signal))
  );
}

function resolveSearchManualRecoveryTimeoutMs(): number {
  const explicit = Number.parseInt(
    process.env.TOUCH_BROWSER_SEARCH_MANUAL_RECOVERY_TIMEOUT_MS ?? "",
    10,
  );
  return Number.isFinite(explicit) && explicit > 0
    ? explicit
    : SEARCH_MANUAL_RECOVERY_TIMEOUT_MS;
}

async function loadPageSource(
  page: Page,
  source: BrowserSource,
): Promise<BrowserLoadDiagnostics> {
  if (source.html) {
    if (source.searchIdentity) {
      const dataUrl = `data:text/html;charset=utf-8,${encodeURIComponent(source.html)}`;
      await page.goto(dataUrl, {
        waitUntil: "load",
        timeout: PAGE_NAVIGATION_TIMEOUT_MS,
      });
    } else {
      await page.setContent(source.html, {
        waitUntil: "load",
        timeout: PAGE_NAVIGATION_TIMEOUT_MS,
      });
    }
    return idleLoadDiagnostics("inline-load");
  }

  if (source.url) {
    await page.goto(source.url, {
      waitUntil: "load",
      timeout: PAGE_NAVIGATION_TIMEOUT_MS,
    });
    if (source.searchIdentity) {
      const startedAt = Date.now();
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
      return {
        waitStrategy: "load+search-profile-settle",
        waitBudgetMs:
          SEARCH_PROFILE_POST_LOAD_IDLE_MS + SEARCH_PROFILE_POST_LOAD_WAIT_MS,
        waitConsumedMs: Date.now() - startedAt,
        waitStopReason: "search-profile-settled",
      };
    }

    return await settleForReadableContent(page);
  }

  return idleLoadDiagnostics("no-load");
}

function sameResolvedUrl(left: string, right: string): boolean {
  try {
    return new URL(left).href === new URL(right).href;
  } catch {
    return left === right;
  }
}

function idleLoadDiagnostics(waitStrategy: string): BrowserLoadDiagnostics {
  return {
    waitStrategy,
    waitBudgetMs: 0,
    waitConsumedMs: 0,
    waitStopReason: "not-needed",
  };
}

async function settleForReadableContent(
  page: Page,
): Promise<BrowserLoadDiagnostics> {
  const startedAt = Date.now();
  let waitBudgetMs = GENERIC_POST_LOAD_PROBE_BUDGET_MS;
  let probe = await readPageQualityProbe(page);

  if (probe.ready) {
    return {
      waitStrategy: "load+quality-probe",
      waitBudgetMs,
      waitConsumedMs: 0,
      waitStopReason: probe.reason,
    };
  }

  let extended = false;
  let deadline = startedAt + waitBudgetMs;
  while (Date.now() < deadline) {
    await ignoreNavigationSettleFailure(
      page.waitForTimeout(GENERIC_POST_LOAD_PROBE_INTERVAL_MS),
      "loadPageSource quality probe wait",
    );
    probe = await readPageQualityProbe(page);
    if (probe.ready) {
      return {
        waitStrategy: "load+quality-probe",
        waitBudgetMs,
        waitConsumedMs: Date.now() - startedAt,
        waitStopReason: probe.reason,
      };
    }
    if (
      !extended &&
      Date.now() - startedAt >= GENERIC_POST_LOAD_PROBE_BUDGET_MS &&
      shouldExtendReadableProbe(probe)
    ) {
      extended = true;
      waitBudgetMs = GENERIC_POST_LOAD_PROBE_MAX_MS;
      deadline = startedAt + waitBudgetMs;
    }
  }

  return {
    waitStrategy: "load+quality-probe",
    waitBudgetMs,
    waitConsumedMs: Date.now() - startedAt,
    waitStopReason: probe.reason,
  };
}

function shouldExtendReadableProbe(probe: PageQualityProbe): boolean {
  return (
    !probe.placeholderDetected &&
    probe.score >= 0.35 &&
    (probe.mainBlockCount >= 1 || probe.shellBlockCount <= 12)
  );
}

async function readPageQualityProbe(page: Page): Promise<PageQualityProbe> {
  const stats = await page.evaluate((placeholderHints) => {
    const mainRoots = Array.from(
      document.querySelectorAll("main, article, [role='main']"),
    );
    const roots = mainRoots.length > 0 ? mainRoots : [document.body];
    const contentSelectors =
      "p, li, blockquote, pre, code, table, h1, h2, h3, h4";
    let mainBlockCount = 0;
    let textLikeBlockCount = 0;
    let mainTextLength = 0;

    for (const root of roots) {
      const blocks = Array.from(root.querySelectorAll(contentSelectors));
      for (const block of blocks) {
        const text = (block.textContent ?? "").replaceAll(/\s+/g, " ").trim();
        if (!text) {
          continue;
        }
        const tagName = block.tagName.toLowerCase();
        const isHeading = /^h[1-4]$/.test(tagName);
        if (
          (isHeading && text.length >= 4) ||
          (!isHeading && text.length >= 32)
        ) {
          mainBlockCount += 1;
          mainTextLength += text.length;
        }
        if (
          ["p", "li", "blockquote", "pre", "code", "table"].includes(tagName)
        ) {
          textLikeBlockCount += 1;
        }
      }
    }

    const shellSelectors =
      "nav a, nav button, header a, header button, footer a, aside a, aside button, [role='navigation'] a, [role='navigation'] button, form input, form button";
    const shellBlockCount = Array.from(
      document.querySelectorAll(shellSelectors),
    ).filter(
      (element) =>
        (element.textContent ?? "").replaceAll(/\s+/g, " ").trim().length > 0 ||
        element.tagName === "INPUT",
    ).length;

    const bodyText = (
      document.body?.innerText ??
      document.body?.textContent ??
      ""
    )
      .replaceAll(/\s+/g, " ")
      .trim();
    const combined = `${document.title} ${bodyText}`.toLowerCase();
    const placeholderDetected = placeholderHints.some((hint) =>
      combined.includes(hint),
    );

    return {
      bodyTextLength: bodyText.length,
      mainBlockCount,
      mainTextLength,
      placeholderDetected,
      shellBlockCount,
      textLikeBlockCount,
    } satisfies PageQualityProbeStats;
  }, JS_PLACEHOLDER_HINTS);

  const shellHeavy = isShellHeavyPageQualityProbe(stats);
  const ready = isReadyPageQualityProbe(stats, shellHeavy);

  return {
    ready,
    placeholderDetected: stats.placeholderDetected,
    reason: pageQualityProbeReason(stats, ready, shellHeavy),
    score: pageQualityProbeScore(stats),
    mainBlockCount: stats.mainBlockCount,
    shellBlockCount: stats.shellBlockCount,
  };
}

function isShellHeavyPageQualityProbe(stats: PageQualityProbeStats): boolean {
  return (
    stats.shellBlockCount >= 10 &&
    stats.mainBlockCount <= 1 &&
    stats.mainTextLength < 220
  );
}

function isReadyPageQualityProbe(
  stats: PageQualityProbeStats,
  shellHeavy: boolean,
): boolean {
  if (stats.placeholderDetected || shellHeavy) {
    return false;
  }

  return (
    (stats.mainBlockCount >= 3 && stats.mainTextLength >= 240) ||
    (stats.mainBlockCount >= 2 && stats.mainTextLength >= 160) ||
    (stats.textLikeBlockCount >= 5 && stats.bodyTextLength >= 700)
  );
}

function pageQualityProbeScore(stats: PageQualityProbeStats): number {
  const score =
    Math.min(stats.mainBlockCount, 8) * 0.09 +
    Math.min(stats.textLikeBlockCount, 8) * 0.06 +
    (Math.min(stats.mainTextLength, 1800) / 1800) * 0.42 -
    (Math.min(stats.shellBlockCount, 18) / 18) *
      (stats.mainBlockCount === 0 ? 0.38 : 0.22) -
    (stats.placeholderDetected ? 0.45 : 0);

  return Math.max(0, Math.min(1, Number(score.toFixed(2))));
}

function pageQualityProbeReason(
  stats: PageQualityProbeStats,
  ready: boolean,
  shellHeavy: boolean,
): string {
  if (ready) {
    return stats.mainBlockCount >= 3
      ? "main-content-ready"
      : "body-content-ready";
  }

  if (stats.placeholderDetected) {
    return "js-placeholder";
  }

  if (shellHeavy) {
    return "shell-heavy";
  }

  return stats.mainBlockCount === 0
    ? "no-main-content"
    : "low-readable-content";
}
