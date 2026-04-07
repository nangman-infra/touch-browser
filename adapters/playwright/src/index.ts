import { execFile } from "node:child_process";
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
import { promisify } from "node:util";

import {
  type BrowserContext,
  type Locator,
  type Page,
  chromium,
} from "playwright";

export type AdapterStatus = {
  readonly status: "ready";
  readonly adapter: "playwright";
  readonly transport: "stdio-json-rpc";
  readonly dynamicFallback: "browser-backed-snapshot";
  readonly browserBackedSnapshot: true;
  readonly capabilities: readonly [
    "adapter.status",
    "browser.snapshot",
    "browser.follow",
    "browser.click",
    "browser.type",
    "browser.submit",
    "browser.paginate",
    "browser.expand",
  ];
};

export type JsonRpcId = string | number | null;

export type JsonRpcRequest = {
  readonly jsonrpc: "2.0";
  readonly id: JsonRpcId;
  readonly method:
    | "adapter.status"
    | "browser.snapshot"
    | "browser.follow"
    | "browser.click"
    | "browser.type"
    | "browser.submit"
    | "browser.paginate"
    | "browser.expand";
  readonly params?: Record<string, unknown>;
};

export type JsonRpcSuccess = {
  readonly jsonrpc: "2.0";
  readonly id: JsonRpcId;
  readonly result: unknown;
};

export type JsonRpcFailure = {
  readonly jsonrpc: "2.0";
  readonly id: JsonRpcId;
  readonly error: {
    readonly code: number;
    readonly message: string;
  };
};

export type JsonRpcResponse = JsonRpcSuccess | JsonRpcFailure;

type BrowserSource = {
  readonly url: string | undefined;
  readonly html: string | undefined;
  readonly contextDir: string | undefined;
  readonly profileDir: string | undefined;
  readonly headless: boolean;
  readonly searchIdentity: boolean;
};

type BrowserPageState = {
  readonly finalUrl: string;
  readonly title: string;
  readonly visibleText: string;
  readonly html: string;
  readonly htmlLength: number;
  readonly linkCount: number;
  readonly buttonCount: number;
  readonly inputCount: number;
  readonly links: Array<{ text: string; href: string | null }>;
};

type TargetDescriptor = {
  readonly text: string | undefined;
  readonly href: string | undefined;
  readonly tagName: string | undefined;
  readonly domPathHint: string | undefined;
  readonly ordinalHint: number | undefined;
  readonly name: string | undefined;
  readonly inputType: string | undefined;
};

type SubmitPrefillDescriptor = {
  readonly targetRef: string;
  readonly targetText: string | undefined;
  readonly targetTagName: string | undefined;
  readonly targetDomPathHint: string | undefined;
  readonly targetOrdinalHint: number | undefined;
  readonly targetName: string | undefined;
  readonly targetInputType: string | undefined;
  readonly value: string;
};

type CandidateDescriptor = {
  readonly locator: Locator;
  readonly domIndex: number;
  readonly text: string;
  readonly href: string | undefined;
  readonly tagName: string;
  readonly fullPath: string;
  readonly parentPath: string;
};

type ScoredCandidate = {
  readonly descriptor: CandidateDescriptor;
  readonly score: number;
};

const CONTEXT_LOCK_TIMEOUT_MS = 30_000;
const CONTEXT_LOCK_RETRY_MS = 150;
const CONTEXT_LOCK_STALE_MS = 120_000;
const SEARCH_PROFILE_MARKER = ".touch-browser-search-profile.json";
const execFileAsync = promisify(execFile);
const SEARCH_USER_AGENT_FALLBACK =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36";
let cachedSearchExecutablePath: Promise<string | undefined> | undefined;
let cachedSearchBrowserVersion: Promise<string | undefined> | undefined;
const EVIDENCE_SELECTOR_KEYWORDS = [
  "platform",
  "operating system",
  "os",
  "architecture",
  "arch",
  "version",
  "package manager",
  "installer",
];

type SearchIdentityProfile = {
  readonly executablePath: string | undefined;
  readonly userAgent: string;
  readonly browserVersion: string;
};

type SearchIdentityBrand = {
  readonly brand: string;
  readonly version: string;
};

type SearchIdentityInitPayload = {
  readonly languages: readonly string[];
  readonly userAgent: string;
  readonly browserVersion: string;
  readonly userAgentDataBrands: readonly SearchIdentityBrand[];
};

type SearchIdentityWebGlPrototype = {
  getParameter(parameter: number): unknown;
};

type SearchIdentityGlobalScope = {
  readonly navigator: Record<string, unknown> & {
    permissions?: {
      query?: (parameters: PermissionDescriptor) => Promise<unknown>;
    };
  };
  readonly Notification: {
    readonly permission: NotificationPermission;
  };
  readonly WebGLRenderingContext?: {
    prototype?: SearchIdentityWebGlPrototype;
  };
  readonly WebGL2RenderingContext?: {
    prototype?: SearchIdentityWebGlPrototype;
  };
  chrome?: unknown;
};

export function adapterStatus(): AdapterStatus {
  return {
    status: "ready",
    adapter: "playwright",
    transport: "stdio-json-rpc",
    dynamicFallback: "browser-backed-snapshot",
    browserBackedSnapshot: true,
    capabilities: [
      "adapter.status",
      "browser.snapshot",
      "browser.follow",
      "browser.click",
      "browser.type",
      "browser.submit",
      "browser.paginate",
      "browser.expand",
    ],
  };
}

export function resetSearchIdentityCachesForTests(): void {
  cachedSearchExecutablePath = undefined;
  cachedSearchBrowserVersion = undefined;
}

export async function hasSearchIdentityMarkerForTests(
  contextDir: string,
): Promise<boolean> {
  return hasSearchIdentityMarker(contextDir);
}

export async function writeSearchIdentityMarkerForTests(
  contextDir: string,
): Promise<void> {
  await writeSearchIdentityMarker(contextDir);
}

export function resolveSearchLocaleForTests(): string {
  return resolveSearchLocale();
}

export async function resolveSearchBrowserVersionForTests(): Promise<
  string | undefined
> {
  return resolveSearchBrowserVersion();
}

export async function resolveSearchUserAgentForTests(): Promise<string> {
  return resolveSearchUserAgent();
}

export function applySearchIdentityToGlobal(
  globalScope: SearchIdentityGlobalScope,
  {
    languages,
    userAgent,
    browserVersion,
    userAgentDataBrands,
  }: SearchIdentityInitPayload,
): void {
  const patch = (target: object, key: PropertyKey, value: unknown) => {
    const define = (receiver: object) => {
      Object.defineProperty(receiver, key, {
        configurable: true,
        get: () => value,
      });
    };
    try {
      define(target);
    } catch {
      try {
        const prototype = Object.getPrototypeOf(target);
        if (prototype) {
          define(prototype);
        }
      } catch {
        // Ignore immutable browser fields.
      }
    }
  };

  patch(globalScope.navigator, "webdriver", undefined);
  patch(globalScope.navigator, "userAgent", userAgent);
  patch(globalScope.navigator, "vendor", "Google Inc.");
  patch(globalScope.navigator, "platform", "MacIntel");
  patch(globalScope.navigator, "hardwareConcurrency", 8);
  patch(globalScope.navigator, "deviceMemory", 8);
  patch(globalScope.navigator, "maxTouchPoints", 0);
  patch(globalScope.navigator, "language", languages[0] ?? "en-US");
  patch(globalScope.navigator, "languages", languages);
  patch(globalScope.navigator, "plugins", [
    { name: "Chrome PDF Plugin", filename: "internal-pdf-viewer" },
    {
      name: "Chrome PDF Viewer",
      filename: "mhjfbmdgcfjbbpaeojofohoefgiehjai",
    },
    { name: "Native Client", filename: "internal-nacl-plugin" },
  ]);
  patch(globalScope.navigator, "mimeTypes", [
    { type: "application/pdf", suffixes: "pdf" },
    { type: "text/pdf", suffixes: "pdf" },
  ]);
  patch(globalScope.navigator, "userAgentData", {
    brands: userAgentDataBrands,
    mobile: false,
    platform: "macOS",
    getHighEntropyValues: async (hints: readonly string[]) => {
      const values: Record<string, unknown> = {
        architecture: "x86",
        bitness: "64",
        mobile: false,
        model: "",
        platform: "macOS",
        platformVersion: "14.0.0",
        uaFullVersion: browserVersion,
        fullVersionList: userAgentDataBrands,
        wow64: false,
      };
      return hints.reduce<Record<string, unknown>>((result, hint) => {
        if (hint in values) {
          result[hint] = values[hint];
        }
        return result;
      }, {});
    },
    toJSON: () => ({
      brands: userAgentDataBrands,
      mobile: false,
      platform: "macOS",
    }),
  });

  const chromeValue = {
    runtime: {},
    app: {},
    loadTimes: () => undefined,
    csi: () => undefined,
  };
  try {
    Object.defineProperty(globalScope, "chrome", {
      configurable: true,
      value: chromeValue,
    });
  } catch {
    try {
      globalScope.chrome = chromeValue;
    } catch {
      // Ignore immutable browser fields.
    }
  }

  const patchWebGl = (prototype: SearchIdentityWebGlPrototype | undefined) => {
    if (!prototype || typeof prototype.getParameter !== "function") {
      return;
    }
    const originalGetParameter = prototype.getParameter;
    prototype.getParameter = function (parameter: number) {
      if (parameter === 37445) {
        return "Intel Inc.";
      }
      if (parameter === 37446) {
        return "Intel Iris OpenGL Engine";
      }
      return originalGetParameter.call(this, parameter);
    };
  };

  patchWebGl(globalScope.WebGLRenderingContext?.prototype);
  patchWebGl(globalScope.WebGL2RenderingContext?.prototype);

  const permissions = globalScope.navigator.permissions;
  if (permissions && typeof permissions.query === "function") {
    const originalQuery = permissions.query.bind(permissions);
    permissions.query = ((parameters: PermissionDescriptor) => {
      if (parameters.name === "notifications") {
        return Promise.resolve({
          name: "notifications",
          state: globalScope.Notification.permission,
          onchange: null,
          addEventListener() {},
          removeEventListener() {},
          dispatchEvent() {
            return false;
          },
        } as unknown as PermissionStatus);
      }
      return originalQuery(parameters);
    }) as typeof permissions.query;
  }
}

function buildSearchIdentityInitScript(
  payload: SearchIdentityInitPayload,
): string {
  return `(${applySearchIdentityToGlobal.toString()})(globalThis, ${JSON.stringify(payload)});`;
}

export async function handleRequest(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  switch (request.method) {
    case "adapter.status":
      return success(request.id, adapterStatus());
    case "browser.snapshot":
      return handleSnapshot(request);
    case "browser.follow":
      return handleFollow(request);
    case "browser.click":
      return handleClick(request);
    case "browser.type":
      return handleType(request);
    case "browser.submit":
      return handleSubmit(request);
    case "browser.paginate":
      return handlePaginate(request);
    case "browser.expand":
      return handleExpand(request);
    default:
      return failure(
        request.id,
        -32601,
        `Unsupported method: ${request.method}`,
      );
  }
}

async function handleSnapshot(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const budget = asNumber(request.params?.budget) ?? 1200;
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);
  const searchIdentity = asBoolean(request.params?.searchIdentity) ?? false;

  if (!url && !html && !contextDir && !profileDir) {
    return failure(
      request.id,
      -32602,
      "browser.snapshot requires `params.url`, `params.html`, `params.contextDir`, or `params.profileDir`.",
    );
  }

  try {
    const pageState = await withPage<BrowserPageState>(
      browserSource(
        url,
        html,
        headless,
        contextDir,
        profileDir,
        searchIdentity,
      ),
      async (page) => {
        await maybeExpandEvidenceSelectors(page);
        return capturePageState(page);
      },
    );
    return success(request.id, {
      status: "ok",
      mode: "playwright-browser",
      requestedBudget: budget,
      source: url ?? (html ? "inline-html" : "persistent-context"),
      ...pageState,
      limitedDynamicActions: [
        "browser.follow",
        "browser.paginate",
        "browser.expand",
      ],
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function handleFollow(request: JsonRpcRequest): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetHref = asString(request.params?.targetHref);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (!targetRef && !targetText && !targetHref) {
    return failure(
      request.id,
      -32602,
      "browser.follow requires `params.targetRef`, `params.targetText`, or `params.targetHref`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.follow requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetHref ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const target = {
          text: targetText,
          href: targetHref,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        } satisfies TargetDescriptor;
        const locator = await findFollowLocator(page, target);
        let clickedText = resolvedTarget;

        if (locator) {
          clickedText = normalizeWhitespace(
            (await locator.textContent().catch(() => resolvedTarget)) ??
              resolvedTarget,
          );
          await locator.click();
          await settleAfterAction(page);
        } else {
          const fallbackUrl = resolveSafeFollowUrl(page.url(), target.href);
          if (!fallbackUrl) {
            throw new Error(
              `No follow target was found for \`${resolvedTarget}\`.`,
            );
          }

          await page.goto(fallbackUrl, { waitUntil: "domcontentloaded" });
          await settleAfterAction(page);
        }

        return {
          status: "ok",
          method: "browser.follow",
          limitedDynamicAction: true,
          followedRef: targetRef ?? targetText ?? targetHref,
          targetText: resolvedTarget,
          targetHref,
          clickedText,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function handleClick(request: JsonRpcRequest): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetHref = asString(request.params?.targetHref);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (!targetRef && !targetText && !targetHref) {
    return failure(
      request.id,
      -32602,
      "browser.click requires `params.targetRef`, `params.targetText`, or `params.targetHref`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.click requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetHref ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const target = {
          text: targetText,
          href: targetHref,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        } satisfies TargetDescriptor;
        const locator = await findClickLocator(page, target);
        let clickedText = resolvedTarget;

        if (locator) {
          clickedText = normalizeWhitespace(
            (await locator.textContent().catch(() => resolvedTarget)) ??
              resolvedTarget,
          );
          await locator.click();
          await settleAfterAction(page);
        } else {
          const fallbackUrl = resolveSafeFollowUrl(page.url(), target.href);
          if (!fallbackUrl) {
            throw new Error(
              `No click target was found for \`${resolvedTarget}\`.`,
            );
          }

          await page.goto(fallbackUrl, { waitUntil: "domcontentloaded" });
          await settleAfterAction(page);
        }

        return {
          status: "ok",
          method: "browser.click",
          limitedDynamicAction: false,
          clickedRef: targetRef ?? targetText ?? targetHref,
          targetText: resolvedTarget,
          targetHref,
          clickedText,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function handleType(request: JsonRpcRequest): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const targetName = asString(request.params?.targetName);
  const targetInputType = asString(request.params?.targetInputType);
  const value = asString(request.params?.value);
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (!targetRef && !targetText) {
    return failure(
      request.id,
      -32602,
      "browser.type requires `params.targetRef` or `params.targetText`.",
    );
  }

  if (!value) {
    return failure(request.id, -32602, "browser.type requires `params.value`.");
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.type requires either `params.url` or `params.html`.",
    );
  }

  try {
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const target = {
          text: targetText,
          href: undefined,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: targetName,
          inputType: targetInputType,
        } satisfies TargetDescriptor;
        const locator = await findTypeLocator(page, target);
        if (!locator) {
          throw new Error(
            `No input target was found for \`${targetRef ?? targetText}\`.`,
          );
        }

        await fillTargetLocator(page, locator, value);
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.type",
          limitedDynamicAction: false,
          typedRef: targetRef ?? targetText,
          targetText: targetText ?? targetRef,
          typedLength: value.length,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function handleSubmit(request: JsonRpcRequest): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);
  const prefill = asSubmitPrefillDescriptors(request.params?.prefill);

  if (!targetRef && !targetText) {
    return failure(
      request.id,
      -32602,
      "browser.submit requires `params.targetRef` or `params.targetText`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.submit requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        for (const action of prefill) {
          const fillTarget = {
            text: action.targetText,
            href: undefined,
            tagName: action.targetTagName,
            domPathHint: action.targetDomPathHint,
            ordinalHint: action.targetOrdinalHint,
            name: action.targetName,
            inputType: action.targetInputType,
          } satisfies TargetDescriptor;
          const fillLocator = await findTypeLocator(page, fillTarget);
          if (!fillLocator) {
            continue;
          }

          await fillTargetLocator(page, fillLocator, action.value);
        }

        const target = {
          text: targetText,
          href: undefined,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        } satisfies TargetDescriptor;
        const locator = await findSubmitLocator(page, target);
        if (!locator) {
          throw new Error(
            `No submit target was found for \`${resolvedTarget}\`.`,
          );
        }

        await submitTargetLocator(locator);
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.submit",
          limitedDynamicAction: false,
          submittedRef: targetRef ?? targetText,
          targetText: resolvedTarget,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function handlePaginate(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const direction = asString(request.params?.direction);
  const currentPage = asNumber(request.params?.currentPage) ?? 1;
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (direction !== "next" && direction !== "prev") {
    return failure(
      request.id,
      -32602,
      "browser.paginate requires `params.direction` to be `next` or `prev`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.paginate requires either `params.url` or `params.html`.",
    );
  }

  try {
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const locator = await findFirstLocator(
          page,
          direction === "next"
            ? nextPaginationSelectors()
            : prevPaginationSelectors(),
        );

        if (!locator) {
          throw new Error(`No ${direction} pagination target was found.`);
        }

        const clickedText = normalizeWhitespace(
          (await locator.textContent().catch(() => direction)) ?? direction,
        );
        await locator.click();
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.paginate",
          limitedDynamicAction: true,
          direction,
          page:
            direction === "next"
              ? currentPage + 1
              : Math.max(1, currentPage - 1),
          clickedText,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function handleExpand(request: JsonRpcRequest): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (!targetRef && !targetText) {
    return failure(
      request.id,
      -32602,
      "browser.expand requires `params.targetRef` or `params.targetText`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.expand requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const locator = await findExpandLocator(page, {
          text: targetText ?? targetRef,
          href: undefined,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        });
        if (!locator) {
          throw new Error(
            `No expandable target was found for \`${resolvedTarget}\`.`,
          );
        }

        const clickedText = normalizeWhitespace(
          (await locator.textContent().catch(() => resolvedTarget)) ??
            resolvedTarget,
        );
        await locator.click();
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.expand",
          limitedDynamicAction: true,
          expandedRef: targetRef ?? targetText,
          targetText: resolvedTarget,
          clickedText,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}

async function readLinks(
  page: Page,
  linkCount: number,
): Promise<Array<{ text: string; href: string | null }>> {
  const links = [];

  for (let index = 0; index < Math.min(linkCount, 10); index += 1) {
    const locator = page.locator("a").nth(index);
    const [text, href] = await Promise.all([
      locator.textContent().catch(() => ""),
      locator.getAttribute("href").catch(() => null),
    ]);
    links.push({
      text: normalizeWhitespace(text ?? ""),
      href,
    });
  }

  return links;
}

async function withPage<T>(
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
        await loadPageSource(page, source);
        return await run(page);
      } finally {
        await closeContextQuietly(context);
      }
    } finally {
      await rm(tempContextDir, { recursive: true, force: true }).catch(
        () => {},
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

async function searchIdentityPersistentOptions(source: BrowserSource) {
  if (!source.searchIdentity) {
    return {};
  }

  const { executablePath, userAgent } =
    await resolveSearchIdentityProfile(source);
  return {
    ...(executablePath ? { executablePath } : {}),
    ignoreDefaultArgs: ["--enable-automation"],
    args: [
      "--disable-blink-features=AutomationControlled",
      "--no-first-run",
      "--no-default-browser-check",
      "--disable-dev-shm-usage",
    ],
    locale: resolveSearchLocale(),
    timezoneId: resolveSearchTimezoneId(),
    userAgent,
  };
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

async function closeContextQuietly(context: BrowserContext): Promise<void> {
  await context.close().catch(() => {});
}

function isAlreadyExistsError(error: unknown): boolean {
  return (
    typeof error === "object" &&
    error !== null &&
    "code" in error &&
    (error as { code?: string }).code === "EEXIST"
  );
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
      await page.goto(dataUrl, { waitUntil: "domcontentloaded" });
    } else {
      await page.setContent(source.html, { waitUntil: "domcontentloaded" });
    }
  } else if (source.url) {
    await page.goto(source.url, { waitUntil: "domcontentloaded" });
    if (source.searchIdentity) {
      await page
        .waitForLoadState("networkidle", { timeout: 3_000 })
        .catch(() => {});
      await page.waitForTimeout(250).catch(() => {});
    }
  }
}

async function capturePageState(page: Page): Promise<BrowserPageState> {
  let lastError: unknown;

  for (let attempt = 0; attempt < 3; attempt += 1) {
    try {
      const title = await page.title();
      const visibleText = normalizeWhitespace(
        await page
          .locator("body")
          .innerText()
          .catch(() => ""),
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

      await page
        .waitForLoadState("domcontentloaded", { timeout: 1000 })
        .catch(() => {});
      await page
        .waitForLoadState("networkidle", { timeout: 1000 })
        .catch(() => {});
      await page.waitForTimeout(150).catch(() => {});
    }
  }

  throw lastError instanceof Error
    ? lastError
    : new Error(
        describeUnknownValue(lastError, "Unknown browser page state error"),
      );
}

async function maybeExpandEvidenceSelectors(page: Page): Promise<void> {
  const seenControls = new Set<string>();
  const selectors = [
    "button[aria-haspopup='listbox'][aria-expanded='false']",
    "[role='combobox'][aria-expanded='false']",
  ];

  for (const selector of selectors) {
    const controlCount = await page
      .locator(selector)
      .count()
      .catch(() => 0);
    for (let index = 0; index < Math.min(controlCount, 4); index += 1) {
      const locator = page.locator(selector).nth(index);
      const descriptor = await selectorDescriptor(locator);
      if (!descriptor || !looksLikeEvidenceSelector(descriptor)) {
        continue;
      }
      const cacheKey = descriptor.toLowerCase();
      if (seenControls.has(cacheKey)) {
        continue;
      }
      seenControls.add(cacheKey);

      const isVisible = await locator.isVisible().catch(() => false);
      if (!isVisible) {
        continue;
      }

      await locator.click().catch(() => {});
      await settleAfterAction(page);
    }
  }
}

async function selectorDescriptor(locator: Locator): Promise<string> {
  const [text, ariaLabel, name] = await Promise.all([
    locator.textContent().catch(() => ""),
    locator.getAttribute("aria-label").catch(() => null),
    locator.getAttribute("name").catch(() => null),
  ]);

  return normalizeWhitespace([text, ariaLabel, name].filter(Boolean).join(" "));
}

function looksLikeEvidenceSelector(text: string): boolean {
  const normalized = text.toLowerCase();
  return EVIDENCE_SELECTOR_KEYWORDS.some((keyword) =>
    normalized.includes(keyword),
  );
}

function browserSource(
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

async function resolveSearchBrowserExecutablePath(): Promise<
  string | undefined
> {
  cachedSearchExecutablePath ??= (async () => {
    const explicitExecutable =
      process.env.TOUCH_BROWSER_SEARCH_CHROME_EXECUTABLE;
    if (explicitExecutable) {
      try {
        await stat(explicitExecutable);
        return explicitExecutable;
      } catch {
        return undefined;
      }
    }

    const candidates = [
      "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
      "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
      "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
      "/usr/bin/google-chrome",
      "/usr/bin/google-chrome-stable",
      "/usr/bin/chromium",
      "/usr/bin/chromium-browser",
    ].filter((candidate): candidate is string => Boolean(candidate));

    for (const candidate of candidates) {
      try {
        await stat(candidate);
        return candidate;
      } catch {
        // Try the next candidate.
      }
    }

    return undefined;
  })();

  return cachedSearchExecutablePath;
}

function searchIdentityMarkerPath(contextDir: string): string {
  return path.join(contextDir, SEARCH_PROFILE_MARKER);
}

async function hasSearchIdentityMarker(contextDir: string): Promise<boolean> {
  try {
    await stat(searchIdentityMarkerPath(contextDir));
    return true;
  } catch {
    return false;
  }
}

async function writeSearchIdentityMarker(contextDir: string): Promise<void> {
  await mkdir(contextDir, { recursive: true });
  await writeFile(
    searchIdentityMarkerPath(contextDir),
    JSON.stringify({ profile: "search", version: 1 }, null, 2),
    "utf8",
  );
}

function resolveSearchLocale(): string {
  const locale = process.env.TOUCH_BROWSER_SEARCH_LOCALE ?? process.env.LANG;
  if (!locale) {
    return "en-US";
  }
  return locale.replace(/\.UTF-8$/i, "").replaceAll("_", "-");
}

function resolveSearchTimezoneId(): string {
  return (
    process.env.TOUCH_BROWSER_SEARCH_TIMEZONE ??
    Intl.DateTimeFormat().resolvedOptions().timeZone
  );
}

function resolveSearchLanguages(): string[] {
  const locale = resolveSearchLocale();
  const languages = new Set<string>([locale]);
  const primary = locale.split("-")[0];
  if (primary && primary !== locale) {
    languages.add(primary);
  }
  languages.add("en-US");
  languages.add("en");
  return Array.from(languages);
}

async function resolveSearchBrowserVersion(): Promise<string | undefined> {
  cachedSearchBrowserVersion ??= (async () => {
    const explicitVersion = process.env.TOUCH_BROWSER_SEARCH_CHROME_VERSION;
    if (explicitVersion) {
      return explicitVersion;
    }

    const executablePath = await resolveSearchBrowserExecutablePath();
    if (!executablePath) {
      return undefined;
    }

    try {
      const { stdout, stderr } = await execFileAsync(executablePath, [
        "--version",
      ]);
      const combined = `${stdout ?? ""} ${stderr ?? ""}`;
      return extractBrowserVersion(combined);
    } catch {
      return undefined;
    }
  })();

  return cachedSearchBrowserVersion;
}

async function resolveSearchUserAgent(): Promise<string> {
  const explicitUserAgent = process.env.TOUCH_BROWSER_SEARCH_USER_AGENT;
  if (explicitUserAgent) {
    return explicitUserAgent;
  }

  const version =
    (await resolveSearchBrowserVersion()) ?? fallbackSearchBrowserVersion();
  return `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version} Safari/537.36`;
}

function fallbackSearchBrowserVersion(): string {
  return (
    SEARCH_USER_AGENT_FALLBACK.match(/Chrome\/([0-9.]+)/)?.[1] ?? "146.0.0.0"
  );
}

function shouldUseDedicatedSearchBrowser(source: BrowserSource): boolean {
  return typeof source.url === "string" && source.url.length > 0;
}

async function resolveSearchIdentityProfile(
  source: BrowserSource,
): Promise<SearchIdentityProfile> {
  const fallbackVersion = fallbackSearchBrowserVersion();
  const explicitUserAgent = process.env.TOUCH_BROWSER_SEARCH_USER_AGENT;

  if (!shouldUseDedicatedSearchBrowser(source)) {
    const browserVersion =
      explicitUserAgent?.match(/Chrome\/([0-9.]+)/)?.[1] ?? fallbackVersion;
    return {
      executablePath: undefined,
      userAgent: explicitUserAgent ?? SEARCH_USER_AGENT_FALLBACK,
      browserVersion,
    };
  }

  const [executablePath, browserVersion, userAgent] = await Promise.all([
    resolveSearchBrowserExecutablePath(),
    resolveSearchBrowserVersion(),
    resolveSearchUserAgent(),
  ]);

  return {
    executablePath,
    userAgent,
    browserVersion: browserVersion ?? fallbackVersion,
  };
}

function buildSearchUserAgentBrands(version: string): Array<{
  brand: string;
  version: string;
}> {
  const majorVersion = version.split(".")[0] ?? "146";
  return [
    { brand: "Not=A?Brand", version: "99" },
    { brand: "Chromium", version: majorVersion },
    { brand: "Google Chrome", version: majorVersion },
  ];
}

async function installSearchIdentity(
  context: BrowserContext,
  source: BrowserSource,
): Promise<void> {
  const languages = resolveSearchLanguages();
  const { userAgent, browserVersion } =
    await resolveSearchIdentityProfile(source);
  const userAgentDataBrands = buildSearchUserAgentBrands(browserVersion);
  await context.addInitScript({
    content: buildSearchIdentityInitScript({
      languages,
      userAgent,
      browserVersion,
      userAgentDataBrands,
    }),
  });
}

function sameResolvedUrl(left: string, right: string): boolean {
  try {
    return new URL(left).href === new URL(right).href;
  } catch {
    return left === right;
  }
}

async function findFirstLocator(
  page: Page,
  selectors: string[],
): Promise<ReturnType<Page["locator"]> | undefined> {
  for (const selector of selectors) {
    const locator = page.locator(selector).first();
    const count = await locator.count().catch(() => 0);
    if (count > 0) {
      return locator;
    }
  }

  return undefined;
}

async function findExpandLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator("button, [role='button'], summary, a"),
    target,
  );
}

async function findFollowLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(page.locator("a"), target);
}

async function findClickLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator(
      "button, [role='button'], a, input[type='submit'], input[type='button'], input[type='checkbox'], input[type='radio']",
    ),
    target,
  );
}

async function findTypeLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator("input, textarea, [contenteditable='true']"),
    target,
  );
}

async function findSubmitLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator(
      "form, button[type='submit'], input[type='submit'], button, input[type='button']",
    ),
    target,
  );
}

async function findBestLocator(
  root: Locator,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  const count = await root.count();
  const candidates: ScoredCandidate[] = [];

  for (let index = 0; index < count; index += 1) {
    const locator = root.nth(index);
    const descriptor = await describeCandidate(locator, index);
    if (!descriptor) {
      continue;
    }

    const score = scoreCandidate(descriptor, target);
    if (score > 0) {
      candidates.push({ descriptor, score });
    }
  }

  candidates.sort((left, right) => {
    const scoreDiff = right.score - left.score;
    if (scoreDiff !== 0) {
      return scoreDiff;
    }

    return left.descriptor.domIndex - right.descriptor.domIndex;
  });

  if (candidates.length === 0) {
    return undefined;
  }

  const firstCandidate = candidates[0];
  if (!firstCandidate) {
    return undefined;
  }

  if (target.ordinalHint && target.ordinalHint > 1) {
    const topScore = firstCandidate.score;
    const topCandidates = candidates.filter(
      (candidate) => candidate.score === topScore,
    );
    const ordinalIndex = target.ordinalHint - 1;
    return (
      topCandidates[ordinalIndex]?.descriptor.locator ??
      candidates[ordinalIndex]?.descriptor.locator ??
      firstCandidate.descriptor.locator
    );
  }

  return firstCandidate.descriptor.locator;
}

async function describeCandidate(
  locator: Locator,
  domIndex: number,
): Promise<CandidateDescriptor | undefined> {
  const isVisible = await locator.isVisible().catch(() => false);
  if (!isVisible) {
    return undefined;
  }

  const [
    text,
    href,
    tagName,
    fullPath,
    parentPath,
    name,
    inputType,
    placeholder,
    value,
    ariaLabel,
  ] = await Promise.all([
    locator.textContent().catch(() => ""),
    locator.getAttribute("href").catch(() => null),
    locator
      .evaluate((element) => element.tagName.toLowerCase())
      .catch(() => ""),
    locator
      .evaluate((element) => {
        const parts: string[] = [];
        let current: Element | null = element;
        while (current) {
          parts.unshift(current.tagName.toLowerCase());
          current = current.parentElement;
        }
        return parts.join(" > ");
      })
      .catch(() => ""),
    locator
      .evaluate((element) => {
        const parts: string[] = [];
        let current: Element | null = element.parentElement;
        while (current) {
          parts.unshift(current.tagName.toLowerCase());
          current = current.parentElement;
        }
        return parts.join(" > ");
      })
      .catch(() => ""),
    locator.getAttribute("name").catch(() => null),
    locator.getAttribute("type").catch(() => null),
    locator.getAttribute("placeholder").catch(() => null),
    locator.inputValue().catch(() => ""),
    locator.getAttribute("aria-label").catch(() => null),
  ]);

  const normalizedText = normalizeWhitespace(text ?? "");
  const inputDescriptor = normalizeWhitespace(
    [name, inputType, placeholder, value, ariaLabel]
      .map((part) => normalizeWhitespace(part ?? ""))
      .filter(Boolean)
      .join(" "),
  );
  const resolvedText = normalizedText || inputDescriptor;
  if (!resolvedText && !href) {
    return undefined;
  }

  return {
    locator,
    domIndex,
    text: resolvedText,
    href: href ?? undefined,
    tagName,
    fullPath,
    parentPath,
  };
}

function scoreCandidate(
  candidate: CandidateDescriptor,
  target: TargetDescriptor,
): number {
  const candidateText = candidate.text.toLowerCase();
  const targetText = normalizeWhitespace(target.text ?? "").toLowerCase();
  const partialScores = [
    scoreTextMatch(candidateText, targetText),
    scoreHrefMatch(candidate.href, target.href),
    scoreTagNameMatch(candidate.tagName, target.tagName),
    scoreContainsMatch(candidateText, target.name, 2),
    scoreContainsMatch(candidateText, target.inputType, 1),
    scoreDomPathMatch(candidate, target.domPathHint),
  ];
  let score = 0;

  for (const partialScore of partialScores) {
    if (partialScore === undefined) {
      return 0;
    }
    score += partialScore;
  }

  return score;
}

async function fillTargetLocator(
  page: Page,
  locator: Locator,
  value: string,
): Promise<void> {
  const tagName = await locator
    .evaluate((element) => element.tagName.toLowerCase())
    .catch(() => "");
  const isContentEditable = await locator
    .evaluate((element) => element.hasAttribute("contenteditable"))
    .catch(() => false);

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

  await locator.dispatchEvent("input").catch(() => {});
  await locator.dispatchEvent("change").catch(() => {});
}

async function submitTargetLocator(locator: Locator): Promise<void> {
  const tagName = await locator
    .evaluate((element) => element.tagName.toLowerCase())
    .catch(() => "");

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

function resolveSafeFollowUrl(
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

function nextPaginationSelectors(): string[] {
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

function prevPaginationSelectors(): string[] {
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

async function settleAfterAction(page: Page): Promise<void> {
  await page
    .waitForLoadState("domcontentloaded", { timeout: 250 })
    .catch(() => {});
  await page.waitForLoadState("networkidle", { timeout: 250 }).catch(() => {});
  await page.waitForTimeout(75);
}

function success(id: JsonRpcId, result: unknown): JsonRpcSuccess {
  return {
    jsonrpc: "2.0",
    id,
    result,
  };
}

function failure(id: JsonRpcId, code: number, message: string): JsonRpcFailure {
  return {
    jsonrpc: "2.0",
    id,
    error: { code, message },
  };
}

function asString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

function asNumber(value: unknown): number | undefined {
  return typeof value === "number" ? value : undefined;
}

function asBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

function asPositiveInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value > 0
    ? value
    : undefined;
}

function asSubmitPrefillDescriptors(value: unknown): SubmitPrefillDescriptor[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((entry) => {
      if (entry === null || typeof entry !== "object") {
        return undefined;
      }

      const record = entry as Record<string, unknown>;
      const targetRef = asString(record.targetRef);
      const textValue = asString(record.value);
      if (!targetRef || !textValue) {
        return undefined;
      }

      return {
        targetRef,
        targetText: asString(record.targetText),
        targetTagName: asString(record.targetTagName),
        targetDomPathHint: asString(record.targetDomPathHint),
        targetOrdinalHint: asPositiveInteger(record.targetOrdinalHint),
        targetName: asString(record.targetName),
        targetInputType: asString(record.targetInputType),
        value: textValue,
      } satisfies SubmitPrefillDescriptor;
    })
    .filter((entry): entry is SubmitPrefillDescriptor => entry !== undefined);
}

function normalizeWhitespace(value: string): string {
  return value.trim().replaceAll(/\s+/g, " ");
}

async function readStdin() {
  const chunks: Buffer[] = [];

  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }

  return Buffer.concat(chunks).toString("utf8").trim();
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const explicitRequest = process.argv[2];
  const input = explicitRequest ?? (await readStdin());

  if (input) {
    const request = JSON.parse(input) as JsonRpcRequest;
    console.log(JSON.stringify(await handleRequest(request), null, 2));
  } else {
    console.log(JSON.stringify(adapterStatus(), null, 2));
  }
}

function describeUnknownValue(value: unknown, fallback: string): string {
  if (value === null || value === undefined) {
    return fallback;
  }

  if (typeof value === "string") {
    return value;
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  if (value instanceof Error) {
    return value.message;
  }

  try {
    const serialized = JSON.stringify(value);
    return serialized ?? fallback;
  } catch {
    return fallback;
  }
}

function extractBrowserVersion(output: string): string | undefined {
  let token = "";

  for (const char of output) {
    if (isAsciiDigit(char) || char === ".") {
      token += char;
      continue;
    }

    const version = normalizeVersionToken(token);
    if (version) {
      return version;
    }
    token = "";
  }

  return normalizeVersionToken(token);
}

function normalizeVersionToken(token: string): string | undefined {
  if (!token.includes(".")) {
    return undefined;
  }

  const parts = token.split(".");
  if (parts.length !== 4) {
    return undefined;
  }

  return parts.every(isDigitString) ? token : undefined;
}

function isDigitString(value: string): boolean {
  return value.length > 0 && Array.from(value).every(isAsciiDigit);
}

function isAsciiDigit(char: string): boolean {
  return char >= "0" && char <= "9";
}

function scoreTextMatch(
  candidateText: string,
  targetText: string,
): number | undefined {
  if (!targetText) {
    return 0;
  }

  if (candidateText === targetText) {
    return 5;
  }

  if (
    candidateText.includes(targetText) ||
    targetText.includes(candidateText)
  ) {
    return 3;
  }

  return undefined;
}

function scoreHrefMatch(
  candidateHref: string | undefined,
  targetHref: string | undefined,
): number | undefined {
  if (!targetHref) {
    return 0;
  }

  if (candidateHref === targetHref) {
    return 4;
  }

  return candidateHref ? undefined : 0;
}

function scoreTagNameMatch(
  candidateTagName: string,
  targetTagName: string | undefined,
): number | undefined {
  if (!targetTagName) {
    return 0;
  }

  return candidateTagName === targetTagName.toLowerCase() ? 2 : undefined;
}

function scoreContainsMatch(
  candidateText: string,
  targetValue: string | undefined,
  score: number,
): number | undefined {
  if (!targetValue) {
    return 0;
  }

  return candidateText.includes(targetValue.toLowerCase()) ? score : undefined;
}

function scoreDomPathMatch(
  candidate: CandidateDescriptor,
  domPathHint: string | undefined,
): number {
  if (!domPathHint) {
    return 0;
  }

  const normalizedHint = domPathHint.toLowerCase();
  if (candidate.parentPath === normalizedHint) {
    return 6;
  }

  if (candidate.fullPath === normalizedHint) {
    return 5;
  }

  return candidate.fullPath.startsWith(`${normalizedHint} >`) ? 2 : 0;
}
