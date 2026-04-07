import { execFile } from "node:child_process";
import { mkdir, stat, writeFile } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import { promisify } from "node:util";

import type { BrowserContext } from "playwright";

import { extractBrowserVersion } from "./shared.js";
import {
  type BrowserSource,
  SEARCH_PROFILE_MARKER,
  type SearchIdentityArchitecture,
  type SearchIdentityGlobalScope,
  type SearchIdentityInitPayload,
  type SearchIdentityPlatformProfile,
  type SearchIdentityProfile,
  type SearchIdentityRuntimeProfile,
} from "./types.js";

const execFileAsync = promisify(execFile);
const SEARCH_BROWSER_FALLBACK_VERSION = ["146", "0", "0", "0"].join(".");

let cachedSearchExecutablePath: Promise<string | undefined> | undefined;
let cachedSearchBrowserVersion: Promise<string | undefined> | undefined;

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

export function searchIdentityPlatformProfileForTests(): SearchIdentityPlatformProfile {
  return searchIdentityPlatformProfile();
}

export function applySearchIdentityToGlobal(
  globalScope: SearchIdentityGlobalScope,
  {
    languages,
    userAgent,
    browserVersion,
    userAgentDataBrands,
    navigatorPlatform,
    userAgentDataPlatform,
    architecture,
    bitness,
    platformVersion,
    webGlVendor,
    webGlRenderer,
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
  patch(globalScope.navigator, "platform", navigatorPlatform);
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
    platform: userAgentDataPlatform,
    getHighEntropyValues: async (hints: readonly string[]) => {
      const values: Record<string, unknown> = {
        architecture,
        bitness,
        mobile: false,
        model: "",
        platform: userAgentDataPlatform,
        platformVersion,
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
      platform: userAgentDataPlatform,
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

  const patchWebGl = (
    prototype: { getParameter(parameter: number): unknown } | undefined,
  ) => {
    if (!prototype || typeof prototype.getParameter !== "function") {
      return;
    }
    const originalGetParameter = prototype.getParameter;
    prototype.getParameter = function (parameter: number) {
      if (parameter === 37445) {
        return webGlVendor;
      }
      if (parameter === 37446) {
        return webGlRenderer;
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

export async function searchIdentityPersistentOptions(
  source: BrowserSource,
): Promise<Record<string, unknown>> {
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

export async function installSearchIdentity(
  context: BrowserContext,
  source: BrowserSource,
): Promise<void> {
  const languages = resolveSearchLanguages();
  const { userAgent, browserVersion, platformProfile } =
    await resolveSearchIdentityProfile(source);
  const userAgentDataBrands = buildSearchUserAgentBrands(browserVersion);
  await context.addInitScript({
    content: buildSearchIdentityInitScript({
      languages,
      userAgent,
      browserVersion,
      userAgentDataBrands,
      navigatorPlatform: platformProfile.navigatorPlatform,
      userAgentDataPlatform: platformProfile.userAgentDataPlatform,
      architecture: platformProfile.architecture,
      bitness: platformProfile.bitness,
      platformVersion: platformProfile.platformVersion,
      webGlVendor: platformProfile.webGlVendor,
      webGlRenderer: platformProfile.webGlRenderer,
    }),
  });
}

export async function hasSearchIdentityMarker(
  contextDir: string,
): Promise<boolean> {
  try {
    await stat(searchIdentityMarkerPath(contextDir));
    return true;
  } catch {
    return false;
  }
}

export async function writeSearchIdentityMarker(
  contextDir: string,
): Promise<void> {
  await mkdir(contextDir, { recursive: true });
  await writeFile(
    searchIdentityMarkerPath(contextDir),
    JSON.stringify({ profile: "search", version: 1 }, null, 2),
    "utf8",
  );
}

function buildSearchIdentityInitScript(
  payload: SearchIdentityInitPayload,
): string {
  return `(${applySearchIdentityToGlobal.toString()})(globalThis, ${JSON.stringify(payload)});`;
}

function resolveSearchIdentityRuntimeProfile(
  runtimeArch: string,
): SearchIdentityRuntimeProfile {
  const architecture: SearchIdentityArchitecture = runtimeArch.startsWith("arm")
    ? "arm"
    : "x86";
  return {
    architecture,
    bitness:
      runtimeArch.includes("64") || runtimeArch === "arm64" ? "64" : "32",
  };
}

function windowsUserAgentPlatformFragment(
  architecture: SearchIdentityArchitecture,
  bitness: string,
): string {
  if (architecture === "arm") {
    return "Windows NT 10.0; Win64; ARM64";
  }
  if (bitness === "64") {
    return "Windows NT 10.0; Win64; x64";
  }
  return "Windows NT 10.0";
}

function windowsWebGlRenderer(
  architecture: SearchIdentityArchitecture,
): string {
  if (architecture === "arm") {
    return "ANGLE (Qualcomm Adreno Direct3D11 vs_5_0 ps_5_0)";
  }
  return "ANGLE (Intel, Intel(R) UHD Graphics Direct3D11 vs_5_0 ps_5_0)";
}

function linuxNavigatorPlatform(
  architecture: SearchIdentityArchitecture,
): string {
  return architecture === "arm" ? "Linux armv8l" : "Linux x86_64";
}

function linuxUserAgentPlatformFragment(
  architecture: SearchIdentityArchitecture,
): string {
  return architecture === "arm" ? "X11; Linux aarch64" : "X11; Linux x86_64";
}

function linuxWebGlRenderer(architecture: SearchIdentityArchitecture): string {
  if (architecture === "arm") {
    return "ANGLE (ARM Mali OpenGL ES)";
  }
  return "ANGLE (Intel, Mesa Intel(R) Graphics OpenGL)";
}

function macUserAgentPlatformFragment(
  architecture: SearchIdentityArchitecture,
): string {
  return architecture === "arm"
    ? "Macintosh; ARM Mac OS X 14_0_0"
    : "Macintosh; Intel Mac OS X 10_15_7";
}

function macWebGlRenderer(architecture: SearchIdentityArchitecture): string {
  return architecture === "arm" ? "Apple GPU" : "Intel Iris OpenGL Engine";
}

function buildWindowsSearchIdentityPlatformProfile(
  runtimeProfile: SearchIdentityRuntimeProfile,
): SearchIdentityPlatformProfile {
  return {
    navigatorPlatform: "Win32",
    userAgentPlatformFragment: windowsUserAgentPlatformFragment(
      runtimeProfile.architecture,
      runtimeProfile.bitness,
    ),
    userAgentDataPlatform: "Windows",
    architecture: runtimeProfile.architecture,
    bitness: runtimeProfile.bitness,
    platformVersion: "15.0.0",
    webGlVendor: "Google Inc. (Microsoft)",
    webGlRenderer: windowsWebGlRenderer(runtimeProfile.architecture),
  };
}

function buildLinuxSearchIdentityPlatformProfile(
  runtimeProfile: SearchIdentityRuntimeProfile,
): SearchIdentityPlatformProfile {
  return {
    navigatorPlatform: linuxNavigatorPlatform(runtimeProfile.architecture),
    userAgentPlatformFragment: linuxUserAgentPlatformFragment(
      runtimeProfile.architecture,
    ),
    userAgentDataPlatform: "Linux",
    architecture: runtimeProfile.architecture,
    bitness: runtimeProfile.bitness,
    platformVersion: "6.0.0",
    webGlVendor: "Google Inc. (Linux)",
    webGlRenderer: linuxWebGlRenderer(runtimeProfile.architecture),
  };
}

function buildMacSearchIdentityPlatformProfile(
  runtimeProfile: SearchIdentityRuntimeProfile,
): SearchIdentityPlatformProfile {
  return {
    navigatorPlatform: "MacIntel",
    userAgentPlatformFragment: macUserAgentPlatformFragment(
      runtimeProfile.architecture,
    ),
    userAgentDataPlatform: "macOS",
    architecture: runtimeProfile.architecture,
    bitness: runtimeProfile.bitness,
    platformVersion: "14.0.0",
    webGlVendor: "Intel Inc.",
    webGlRenderer: macWebGlRenderer(runtimeProfile.architecture),
  };
}

function searchIdentityPlatformProfile(): SearchIdentityPlatformProfile {
  const runtimePlatform = os.platform();
  const runtimeProfile = resolveSearchIdentityRuntimeProfile(os.arch());

  switch (runtimePlatform) {
    case "win32":
      return buildWindowsSearchIdentityPlatformProfile(runtimeProfile);
    case "linux":
      return buildLinuxSearchIdentityPlatformProfile(runtimeProfile);
    default:
      return buildMacSearchIdentityPlatformProfile(runtimeProfile);
  }
}

function buildSearchUserAgent(version: string): string {
  const profile = searchIdentityPlatformProfile();
  return `Mozilla/5.0 (${profile.userAgentPlatformFragment}) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/${version} Safari/537.36`;
}

function currentSearchUserAgentFallback(): string {
  return buildSearchUserAgent(SEARCH_BROWSER_FALLBACK_VERSION);
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
  return buildSearchUserAgent(version);
}

function fallbackSearchBrowserVersion(): string {
  return (
    extractChromeVersionFromUserAgent(currentSearchUserAgentFallback()) ??
    SEARCH_BROWSER_FALLBACK_VERSION
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
  const platformProfile = searchIdentityPlatformProfile();

  if (!shouldUseDedicatedSearchBrowser(source)) {
    const browserVersion =
      extractChromeVersionFromUserAgent(explicitUserAgent) ?? fallbackVersion;
    return {
      executablePath: undefined,
      userAgent: explicitUserAgent ?? currentSearchUserAgentFallback(),
      browserVersion,
      platformProfile,
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
    platformProfile,
  };
}

function extractChromeVersionFromUserAgent(
  userAgent: string | undefined,
): string | undefined {
  return /Chrome\/([0-9.]+)/.exec(userAgent ?? "")?.[1];
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
