import { describe, expect, it } from "vitest";

import {
  expectJsonRpcSuccess,
  handleRequest,
  readVisibleText,
} from "../support/adapter-helpers.js";

describe("playwright adapter browser snapshots", () => {
  it("captures browser-backed snapshots from inline html", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-2",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <head><title>Inline Snapshot</title></head>
            <body>
              <main>
                <h1>Inline Snapshot</h1>
                <p>Browser-backed snapshots should return visible text.</p>
                <a href="/docs">Docs</a>
                <button>Expand</button>
              </main>
            </body>
          </html>
        `,
        budget: 900,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-2",
      result: {
        status: "ok",
        mode: "playwright-browser",
        requestedBudget: 900,
        source: "inline-html",
        title: "Inline Snapshot",
        linkCount: 1,
        buttonCount: 1,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      visibleText: expect.stringContaining(
        "Browser-backed snapshots should return visible text.",
      ),
      links: [{ text: "Docs", href: "/docs" }],
    });
  });

  it("hardens search snapshots to look less like automation", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-identity",
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
                    String(navigator.webdriver),
                    typeof window.chrome,
                    navigator.userAgent.includes("HeadlessChrome"),
                  ].join("|");
                </script>
              </main>
            </body>
          </html>
        `,
        budget: 600,
        searchIdentity: true,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-search-identity",
      result: {
        status: "ok",
        mode: "playwright-browser",
      },
    });

    const visibleText = readVisibleText(response);
    expect(visibleText.endsWith("|object|false")).toBe(true);
    expect(visibleText.startsWith("true|")).toBe(false);
  }, 15_000);

  it("waits for manual recovery pages to clear before capturing a search snapshot", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-manual-recovery",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <head><title>I'm not a robot</title></head>
            <body>
              <main id="app">I'm not a robot</main>
              <script>
                setTimeout(() => {
                  document.title = "Recovered Search";
                  const app = document.getElementById("app");
                  if (app) {
                    app.innerHTML = [
                      '<a href="https://developers.cloudflare.com/workers/platform/pricing/">Cloudflare Workers Pricing</a>',
                      '<p>Pricing details loaded after the checkpoint cleared.</p>',
                    ].join("");
                  }
                }, 600);
              </script>
            </body>
          </html>
        `,
        budget: 600,
        searchIdentity: true,
        manualRecovery: true,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      title: "Recovered Search",
      visibleText: expect.stringContaining(
        "Pricing details loaded after the checkpoint cleared.",
      ),
      links: [
        {
          text: "Cloudflare Workers Pricing",
          href: "https://developers.cloudflare.com/workers/platform/pricing/",
        },
      ],
    });
  }, 15_000);

  it("returns the challenge page immediately when manual recovery is disabled", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-manual-recovery-disabled",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <head><title>I'm not a robot</title></head>
            <body>
              <main id="app">I'm not a robot</main>
              <script>
                setTimeout(() => {
                  document.title = "Recovered Search";
                  const app = document.getElementById("app");
                  if (app) {
                    app.textContent = "Recovered result";
                  }
                }, 600);
              </script>
            </body>
          </html>
        `,
        budget: 600,
        searchIdentity: true,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      title: "I'm not a robot",
      visibleText: expect.stringContaining("I'm not a robot"),
    });
  }, 15_000);

  it("expands obvious selector controls before capturing snapshot evidence", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-platform-selector",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <head><title>Downloads</title></head>
            <body>
              <main>
                <h1>Downloads</h1>
                <button
                  id="platform-trigger"
                  type="button"
                  aria-label="Platform"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                  aria-controls="platform-options"
                >
                  Linux
                </button>
                <ul id="platform-options" role="listbox" hidden>
                  <li role="option">macOS</li>
                  <li role="option">Windows</li>
                  <li role="option">Linux</li>
                </ul>
              </main>
              <script>
                const trigger = document.getElementById("platform-trigger");
                const list = document.getElementById("platform-options");
                trigger?.addEventListener("click", () => {
                  trigger.setAttribute("aria-expanded", "true");
                  list.hidden = false;
                });
              </script>
            </body>
          </html>
        `,
        budget: 700,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      title: "Downloads",
      visibleText: expect.stringContaining("macOS"),
      html: expect.stringContaining('aria-expanded="true"'),
    });
  }, 15_000);

  it("captures more than ten visible links from dense result pages", async () => {
    const links = Array.from({ length: 12 }, (_, index) => {
      const rank = index + 1;
      return `<a href="/result-${rank}">Result ${rank}</a>`;
    }).join("");
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-many-links",
      method: "browser.snapshot",
      params: {
        html: `<!doctype html><html><body><main>${links}</main></body></html>`,
        budget: 700,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      linkCount: 12,
      links: expect.arrayContaining([
        { text: "Result 1", href: "/result-1" },
        { text: "Result 12", href: "/result-12" },
      ]),
    });
    expect(((success.result as { links?: unknown[] }).links ?? []).length).toBe(
      12,
    );
  });

  it("preserves multiple selector option popups as snapshot evidence", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-multi-selector",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <div
                id="overlay"
                hidden
                style="position: fixed; inset: 0; background: transparent;"
              ></div>
              <main>
                <button
                  id="os-trigger"
                  type="button"
                  aria-label="Operating System"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                  aria-controls="os-options"
                >
                  Linux
                </button>
                <div id="os-options" role="listbox" hidden>
                  <style>.decorative-style { color: red; }</style>
                  <div role="option">Windows</div>
                  <div role="option">macOS</div>
                  <div role="option">Linux</div>
                </div>
                <button
                  id="platform-trigger"
                  type="button"
                  aria-label="Platform"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                  aria-controls="platform-options"
                >
                  x64
                </button>
                <div id="platform-options" role="listbox" hidden>
                  <style>.decorative-style { color: blue; }</style>
                  <div role="option">x64</div>
                  <div role="option">arm64</div>
                </div>
              </main>
              <script>
                let openPopup = null;
                const overlay = document.getElementById("overlay");
                const closePopup = () => {
                  if (!openPopup) {
                    return;
                  }

                  const trigger = document.querySelector(
                    '[aria-controls="' + openPopup.id + '"]',
                  );
                  if (trigger) {
                    trigger.setAttribute("aria-expanded", "false");
                  }
                  openPopup.hidden = true;
                  openPopup = null;
                  overlay.hidden = true;
                };

                document.addEventListener("keydown", (event) => {
                  if (event.key === "Escape") {
                    closePopup();
                  }
                });

                for (const id of ["os", "platform"]) {
                  const trigger = document.getElementById(id + "-trigger");
                  const popup = document.getElementById(id + "-options");
                  trigger?.addEventListener("click", () => {
                    if (openPopup && openPopup !== popup) {
                      return;
                    }
                    trigger.setAttribute("aria-expanded", "true");
                    popup.hidden = false;
                    openPopup = popup;
                    overlay.hidden = false;
                  });
                }
              </script>
            </body>
          </html>
        `,
        budget: 700,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      visibleText: expect.stringContaining("macOS"),
      html: expect.stringContaining(
        "touch-browser-evidence-popup-platform-options",
      ),
    });
    expect(
      (success.result as { visibleText?: string }).visibleText ?? "",
    ).toContain("arm64");
  }, 15_000);

  it("skips invisible or missing-popup selectors while preserving valid evidence selectors", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-selector-filtering",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <button
                  type="button"
                  aria-label="Platform"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                  style="display:none"
                >
                  Hidden
                </button>
                <button
                  id="missing-trigger"
                  type="button"
                  aria-label="Operating system"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                >
                  Missing popup
                </button>
                <button
                  id="valid-trigger"
                  type="button"
                  aria-label="Package manager"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                  aria-controls="package-options"
                >
                  npm
                </button>
                <div id="package-options" role="listbox" hidden>
                  <div role="option">npm</div>
                  <div role="option">pnpm</div>
                </div>
              </main>
              <script>
                const validTrigger = document.getElementById("valid-trigger");
                const popup = document.getElementById("package-options");
                validTrigger?.addEventListener("click", () => {
                  validTrigger.setAttribute("aria-expanded", "true");
                  popup.hidden = false;
                });
                document.addEventListener("keydown", (event) => {
                  if (event.key === "Escape") {
                    validTrigger?.setAttribute("aria-expanded", "false");
                    popup.hidden = true;
                  }
                });
              </script>
            </body>
          </html>
        `,
        budget: 700,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      visibleText: expect.stringContaining("pnpm"),
      html: expect.stringContaining(
        "touch-browser-evidence-popup-package-options",
      ),
    });
    expect((success.result as { html?: string }).html ?? "").not.toContain(
      "touch-browser-evidence-popup-missing",
    );
  }, 15_000);

  it("falls back to an outside click when escape does not close an evidence popup", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-selector-close-fallback",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <button
                  id="platform-trigger"
                  type="button"
                  aria-label="Platform"
                  aria-haspopup="listbox"
                  aria-expanded="false"
                  aria-controls="platform-options"
                >
                  x64
                </button>
                <div
                  id="platform-options"
                  role="listbox"
                  hidden
                  data-state="closed"
                >
                  <div role="option">x64</div>
                  <div role="option">arm64</div>
                </div>
              </main>
              <script>
                const trigger = document.getElementById("platform-trigger");
                const popup = document.getElementById("platform-options");
                trigger?.addEventListener("click", () => {
                  trigger.setAttribute("aria-expanded", "true");
                  popup.hidden = false;
                  popup.dataset.state = "open";
                });
                document.body.addEventListener("click", (event) => {
                  if (event.target === document.body) {
                    trigger?.setAttribute("aria-expanded", "false");
                    popup.hidden = true;
                    popup.dataset.state = "closed";
                  }
                });
              </script>
            </body>
          </html>
        `,
        budget: 700,
      },
    });

    const success = expectJsonRpcSuccess(response);
    expect(success.result).toMatchObject({
      visibleText: expect.stringContaining("arm64"),
      html: expect.stringContaining(
        "touch-browser-evidence-popup-platform-options",
      ),
    });
  }, 15_000);
});
