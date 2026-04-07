import type { Locator } from "playwright";

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

export type BrowserSource = {
  readonly url: string | undefined;
  readonly html: string | undefined;
  readonly contextDir: string | undefined;
  readonly profileDir: string | undefined;
  readonly headless: boolean;
  readonly searchIdentity: boolean;
};

export type BrowserPageState = {
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

export type TargetDescriptor = {
  readonly text: string | undefined;
  readonly href: string | undefined;
  readonly tagName: string | undefined;
  readonly domPathHint: string | undefined;
  readonly ordinalHint: number | undefined;
  readonly name: string | undefined;
  readonly inputType: string | undefined;
};

export type SubmitPrefillDescriptor = {
  readonly targetRef: string;
  readonly targetText: string | undefined;
  readonly targetTagName: string | undefined;
  readonly targetDomPathHint: string | undefined;
  readonly targetOrdinalHint: number | undefined;
  readonly targetName: string | undefined;
  readonly targetInputType: string | undefined;
  readonly value: string;
};

export type CandidateDescriptor = {
  readonly locator: Locator;
  readonly domIndex: number;
  readonly text: string;
  readonly href: string | undefined;
  readonly tagName: string;
  readonly fullPath: string;
  readonly parentPath: string;
};

export type ScoredCandidate = {
  readonly descriptor: CandidateDescriptor;
  readonly score: number;
};

export const CONTEXT_LOCK_TIMEOUT_MS = 30_000;
export const CONTEXT_LOCK_RETRY_MS = 150;
export const CONTEXT_LOCK_STALE_MS = 120_000;
export const PAGE_NAVIGATION_TIMEOUT_MS = 15_000;
export const PAGE_ACTION_TIMEOUT_MS = 10_000;
export const ACTION_SETTLE_TIMEOUT_MS = 1_500;
export const ACTION_SETTLE_IDLE_TIMEOUT_MS = 1_250;
export const ACTION_SETTLE_EXTRA_WAIT_MS = 700;
export const SEARCH_PROFILE_POST_LOAD_IDLE_MS = 3_000;
export const SEARCH_PROFILE_POST_LOAD_WAIT_MS = 350;
export const MAX_CAPTURED_LINKS = 50;
export const MAX_EVIDENCE_SELECTOR_CANDIDATES = 8;
export const SEARCH_PROFILE_MARKER = ".touch-browser-search-profile.json";
export const EVIDENCE_SELECTOR_KEYWORDS = [
  "platform",
  "operating system",
  "os",
  "architecture",
  "arch",
  "version",
  "package manager",
  "installer",
] as const;

export type SearchIdentityProfile = {
  readonly executablePath: string | undefined;
  readonly userAgent: string;
  readonly browserVersion: string;
  readonly platformProfile: SearchIdentityPlatformProfile;
};

export type SearchIdentityBrand = {
  readonly brand: string;
  readonly version: string;
};

export type SearchIdentityInitPayload = {
  readonly languages: readonly string[];
  readonly userAgent: string;
  readonly browserVersion: string;
  readonly userAgentDataBrands: readonly SearchIdentityBrand[];
  readonly navigatorPlatform: string;
  readonly userAgentDataPlatform: string;
  readonly architecture: string;
  readonly bitness: string;
  readonly platformVersion: string;
  readonly webGlVendor: string;
  readonly webGlRenderer: string;
};

export type SearchIdentityWebGlPrototype = {
  getParameter(parameter: number): unknown;
};

export type SearchIdentityGlobalScope = {
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

export type SearchIdentityPlatformProfile = {
  readonly navigatorPlatform: string;
  readonly userAgentPlatformFragment: string;
  readonly userAgentDataPlatform: string;
  readonly architecture: string;
  readonly bitness: string;
  readonly platformVersion: string;
  readonly webGlVendor: string;
  readonly webGlRenderer: string;
};

export type EvidencePopupSnapshot = {
  readonly id: string;
  readonly label: string;
  readonly html: string;
};

export type SearchIdentityArchitecture = "arm" | "x86";

export type SearchIdentityRuntimeProfile = {
  readonly architecture: SearchIdentityArchitecture;
  readonly bitness: "32" | "64";
};

export type EvidenceSelectorTarget = {
  readonly locator: Locator;
  readonly popupId: string;
  readonly descriptor: string;
};

export type EvidenceSelectorCandidate = {
  readonly index: number;
  readonly descriptor: string;
  readonly cacheKey: string;
  readonly popupId: string | null;
};
