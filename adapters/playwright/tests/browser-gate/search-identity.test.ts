import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it, vi } from "vitest";

import {
  applySearchIdentityToGlobal,
  handleRequest,
  hasSearchIdentityMarkerForTests,
  readVisibleText,
  resetSearchIdentityCachesForTests,
  resolveSearchBrowserVersionForTests,
  resolveSearchLocaleForTests,
  resolveSearchUserAgentForTests,
  writeSearchIdentityMarkerForTests,
} from "../support/adapter-helpers.js";

async function withEnvironment<T>(
  overrides: Record<string, string | undefined>,
  run: () => Promise<T>,
): Promise<T> {
  const previous = new Map<string, string | undefined>();
  for (const [key, value] of Object.entries(overrides)) {
    previous.set(key, process.env[key]);
    if (value === undefined) {
      delete process.env[key];
    } else {
      process.env[key] = value;
    }
  }

  try {
    resetSearchIdentityCachesForTests();
    return await run();
  } finally {
    for (const [key, value] of previous.entries()) {
      if (value === undefined) {
        delete process.env[key];
      } else {
        process.env[key] = value;
      }
    }
    resetSearchIdentityCachesForTests();
  }
}

describe("playwright adapter search identity coverage", () => {
  it("hardens url-based search snapshots with the dedicated search profile path", async () => {
    const html = `
      <!doctype html>
      <html>
        <body>
          <main>
            <div id="result"></div>
            <script>
              const result = document.getElementById("result");
              result.textContent = [
                String(navigator.webdriver),
                typeof window.chrome,
                navigator.userAgent.includes("HeadlessChrome"),
                location.protocol,
              ].join("|");
            </script>
          </main>
        </body>
      </html>
    `;
    const url = `data:text/html;charset=utf-8,${encodeURIComponent(html)}`;

    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-identity-url",
      method: "browser.snapshot",
      params: {
        url,
        budget: 600,
        searchIdentity: true,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-search-identity-url",
      result: {
        status: "ok",
        mode: "playwright-browser",
        source: url,
      },
    });

    const visibleText = readVisibleText(response);
    expect(visibleText.endsWith("|object|false|data:")).toBe(true);
    expect(visibleText.startsWith("true|")).toBe(false);
  }, 15_000);

  it("persists search identity markers for reusable context directories", async () => {
    const contextDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-search-marker-gate-"),
    );
    const html = `
      <!doctype html>
      <html>
        <body>
          <main>
            <div id="result"></div>
            <script>
              const result = document.getElementById("result");
              result.textContent = [
                String(navigator.webdriver),
                typeof window.chrome,
              ].join("|");
            </script>
          </main>
        </body>
      </html>
    `;

    const first = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-marker-first",
      method: "browser.snapshot",
      params: {
        html,
        contextDir,
        searchIdentity: true,
      },
    });
    const second = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-marker-second",
      method: "browser.snapshot",
      params: {
        html,
        contextDir,
      },
    });

    expect(first).toMatchObject({
      jsonrpc: "2.0",
      id: "req-search-marker-first",
      result: {
        status: "ok",
      },
    });
    expect(second).toMatchObject({
      jsonrpc: "2.0",
      id: "req-search-marker-second",
      result: {
        status: "ok",
      },
    });
    expect(readVisibleText(second).endsWith("|object")).toBe(true);
    expect(readVisibleText(second).startsWith("true|")).toBe(false);
  }, 15_000);

  it("falls back to en-US search locale when no locale environment is configured", async () => {
    await withEnvironment(
      {
        LANG: undefined,
        TOUCH_BROWSER_SEARCH_LOCALE: undefined,
      },
      async () => {
        const response = await handleRequest({
          jsonrpc: "2.0",
          id: "req-search-locale-fallback",
          method: "browser.snapshot",
          params: {
            html: `
              <!doctype html>
              <html>
                <body>
                  <main>
                    <div id="result"></div>
                    <script>
                      const result = document.getElementById("result");
                      result.textContent = [
                        navigator.language,
                        navigator.languages.join(","),
                      ].join("|");
                    </script>
                  </main>
                </body>
              </html>
            `,
            searchIdentity: true,
          },
        });

        expect(response).toMatchObject({
          jsonrpc: "2.0",
          id: "req-search-locale-fallback",
          result: {
            status: "ok",
          },
        });
        expect(readVisibleText(response)).toContain("en-US|en-US,en");
      },
    );
  }, 15_000);

  it("uses an explicit search chrome version for dedicated url-based profiles", async () => {
    await withEnvironment(
      {
        TOUCH_BROWSER_SEARCH_CHROME_VERSION: "123.4.5.6",
        TOUCH_BROWSER_SEARCH_USER_AGENT: undefined,
      },
      async () => {
        const html = `
          <!doctype html>
          <html>
            <body>
              <main>
                <div id="result"></div>
                <script>
                  const result = document.getElementById("result");
                  result.textContent = [
                    navigator.userAgent.includes("Chrome/123.4.5.6"),
                    navigator.userAgent,
                  ].join("|");
                </script>
              </main>
            </body>
          </html>
        `;
        const url = `data:text/html;charset=utf-8,${encodeURIComponent(html)}`;
        const response = await handleRequest({
          jsonrpc: "2.0",
          id: "req-search-version-override",
          method: "browser.snapshot",
          params: {
            url,
            searchIdentity: true,
          },
        });

        expect(response).toMatchObject({
          jsonrpc: "2.0",
          id: "req-search-version-override",
          result: {
            status: "ok",
            mode: "playwright-browser",
          },
        });
        expect(readVisibleText(response).startsWith("true|")).toBe(true);
      },
    );
  }, 15_000);

  it("exposes search identity marker utilities for deterministic coverage", async () => {
    const contextDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-search-marker-unit-"),
    );

    await expect(hasSearchIdentityMarkerForTests(contextDir)).resolves.toBe(
      false,
    );
    await writeSearchIdentityMarkerForTests(contextDir);
    await expect(hasSearchIdentityMarkerForTests(contextDir)).resolves.toBe(
      true,
    );
  });

  it("resolves search locale and browser identity utilities from environment overrides", async () => {
    await withEnvironment(
      {
        LANG: undefined,
        TOUCH_BROWSER_SEARCH_LOCALE: undefined,
      },
      async () => {
        expect(resolveSearchLocaleForTests()).toBe("en-US");
      },
    );

    await withEnvironment(
      {
        TOUCH_BROWSER_SEARCH_LOCALE: "ko_KR.UTF-8",
      },
      async () => {
        expect(resolveSearchLocaleForTests()).toBe("ko-KR");
      },
    );

    await withEnvironment(
      {
        TOUCH_BROWSER_SEARCH_CHROME_VERSION: "123.4.5.6",
        TOUCH_BROWSER_SEARCH_CHROME_EXECUTABLE: undefined,
        TOUCH_BROWSER_SEARCH_USER_AGENT: undefined,
      },
      async () => {
        await expect(resolveSearchBrowserVersionForTests()).resolves.toBe(
          "123.4.5.6",
        );
        await expect(resolveSearchUserAgentForTests()).resolves.toContain(
          "Chrome/123.4.5.6",
        );
      },
    );

    await withEnvironment(
      {
        TOUCH_BROWSER_SEARCH_CHROME_EXECUTABLE: "/bin/false",
        TOUCH_BROWSER_SEARCH_CHROME_VERSION: undefined,
        TOUCH_BROWSER_SEARCH_USER_AGENT: undefined,
      },
      async () => {
        await expect(resolveSearchBrowserVersionForTests()).resolves.toBe(
          undefined,
        );
        await expect(resolveSearchUserAgentForTests()).resolves.toContain(
          "Chrome/146.0.0.0",
        );
      },
    );

    await withEnvironment(
      {
        TOUCH_BROWSER_SEARCH_USER_AGENT: "CustomAgent Chrome/999.0.0.0",
      },
      async () => {
        await expect(resolveSearchUserAgentForTests()).resolves.toBe(
          "CustomAgent Chrome/999.0.0.0",
        );
      },
    );
  });

  it("applies search identity overrides to a browser-like global", async () => {
    const delegatedQuery = vi.fn(async (parameters: PermissionDescriptor) => ({
      name: parameters.name,
      state: "prompt",
    }));
    const navigatorTarget: Record<string, unknown> & {
      permissions: {
        query(parameters: PermissionDescriptor): Promise<unknown>;
      };
    } = {
      permissions: {
        query: delegatedQuery,
      },
    };
    const webGlPrototype = {
      getParameter: vi.fn((parameter: number) => `webgl:${parameter}`),
    };
    const webGl2Prototype = {
      getParameter: vi.fn((parameter: number) => `webgl2:${parameter}`),
    };

    applySearchIdentityToGlobal(
      {
        navigator: navigatorTarget,
        Notification: { permission: "granted" },
        WebGLRenderingContext: { prototype: webGlPrototype },
        WebGL2RenderingContext: { prototype: webGl2Prototype },
      },
      {
        languages: ["en-US", "en"],
        userAgent:
          "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        browserVersion: "146.0.0.0",
        userAgentDataBrands: [
          { brand: "Not=A?Brand", version: "99" },
          { brand: "Chromium", version: "146" },
          { brand: "Google Chrome", version: "146" },
        ],
      },
    );

    expect(navigatorTarget.webdriver).toBeUndefined();
    expect(navigatorTarget.userAgent).toContain("Chrome/146.0.0.0");
    expect(navigatorTarget.vendor).toBe("Google Inc.");
    expect(navigatorTarget.language).toBe("en-US");
    expect(navigatorTarget.languages).toEqual(["en-US", "en"]);
    expect(navigatorTarget.plugins).toHaveLength(3);
    expect(navigatorTarget.mimeTypes).toHaveLength(2);
    expect(
      (
        navigatorTarget.userAgentData as {
          toJSON(): unknown;
          getHighEntropyValues(hints: readonly string[]): Promise<unknown>;
        }
      ).toJSON(),
    ).toMatchObject({
      mobile: false,
      platform: "macOS",
    });
    await expect(
      (
        navigatorTarget.userAgentData as {
          getHighEntropyValues(hints: readonly string[]): Promise<unknown>;
        }
      ).getHighEntropyValues(["uaFullVersion", "platform", "missing"]),
    ).resolves.toEqual({
      platform: "macOS",
      uaFullVersion: "146.0.0.0",
    });
    expect(webGlPrototype.getParameter(37445)).toBe("Intel Inc.");
    expect(webGlPrototype.getParameter(37446)).toBe("Intel Iris OpenGL Engine");
    expect(webGlPrototype.getParameter(7)).toBe("webgl:7");
    expect(webGl2Prototype.getParameter(7)).toBe("webgl2:7");
    await expect(
      navigatorTarget.permissions.query({
        name: "notifications",
      } as PermissionDescriptor),
    ).resolves.toMatchObject({
      name: "notifications",
      state: "granted",
    });
    await expect(
      navigatorTarget.permissions.query({
        name: "geolocation",
      } as PermissionDescriptor),
    ).resolves.toEqual({
      name: "geolocation",
      state: "prompt",
    });
    expect(delegatedQuery).toHaveBeenCalledTimes(1);
  });

  it("falls back to prototype and assignment patching for immutable globals", async () => {
    const delegatedQuery = vi.fn(async (parameters: PermissionDescriptor) => ({
      name: parameters.name,
      state: "prompt",
    }));
    const navigatorPrototype = {};
    const navigatorTarget = Object.create(navigatorPrototype) as {
      permissions: {
        query(parameters: PermissionDescriptor): Promise<unknown>;
      };
      readonly userAgent?: string;
    };
    Object.defineProperty(navigatorTarget, "permissions", {
      configurable: true,
      value: {
        query: delegatedQuery,
      },
    });
    Object.preventExtensions(navigatorTarget);

    const globalScope: Record<string, unknown> & {
      navigator: typeof navigatorTarget;
      Notification: {
        permission: NotificationPermission;
      };
      chrome?: unknown;
    } = {
      navigator: navigatorTarget,
      Notification: { permission: "default" as NotificationPermission },
    };
    Object.defineProperty(globalScope, "chrome", {
      configurable: false,
      value: undefined,
      writable: true,
    });

    applySearchIdentityToGlobal(globalScope, {
      languages: ["ko-KR", "ko"],
      userAgent:
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
      browserVersion: "146.0.0.0",
      userAgentDataBrands: [{ brand: "Chromium", version: "146" }],
    });

    expect(navigatorTarget.userAgent).toContain("Chrome/146.0.0.0");
    expect(
      Object.getOwnPropertyDescriptor(navigatorPrototype, "userAgent"),
    ).toBeDefined();
    expect(globalScope.chrome).toMatchObject({
      runtime: {},
      app: {},
    });
    await expect(
      navigatorTarget.permissions.query({
        name: "notifications",
      } as PermissionDescriptor),
    ).resolves.toMatchObject({
      state: "default",
    });
  });
});
