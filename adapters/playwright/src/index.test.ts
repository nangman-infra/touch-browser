import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { adapterStatus, handleRequest } from "./index.js";

describe("playwright adapter", () => {
  it("reports the fixed transport contract and capabilities", () => {
    expect(adapterStatus()).toEqual({
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
    });
  });

  it("handles adapter status requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-1",
      method: "adapter.status",
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-1",
      result: {
        status: "ready",
        adapter: "playwright",
      },
    });
  });

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
    if ("result" in response) {
      expect(response.result).toMatchObject({
        visibleText: expect.stringContaining(
          "Browser-backed snapshots should return visible text.",
        ),
        links: [{ text: "Docs", href: "/docs" }],
      });
    }
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
    if ("result" in response) {
      const visibleText = String(
        (response.result as { visibleText?: unknown }).visibleText ?? "",
      );
      expect(visibleText.endsWith("|object")).toBe(true);
      expect(visibleText.startsWith("true|")).toBe(false);
    }
  });

  it("serializes concurrent persistent-context requests for the same session directory", async () => {
    const contextDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-playwright-lock-"),
    );
    const html = `
      <!doctype html>
      <html>
        <body>
          <main>
            <h1>Concurrent Context</h1>
            <p>Persistent context access should serialize cleanly.</p>
          </main>
        </body>
      </html>
    `;

    const [first, second] = await Promise.all([
      handleRequest({
        jsonrpc: "2.0",
        id: "req-lock-1",
        method: "browser.snapshot",
        params: {
          html,
          contextDir,
          budget: 600,
          headless: true,
        },
      }),
      handleRequest({
        jsonrpc: "2.0",
        id: "req-lock-2",
        method: "browser.snapshot",
        params: {
          html,
          contextDir,
          budget: 600,
          headless: true,
        },
      }),
    ]);

    expect(first).toMatchObject({
      jsonrpc: "2.0",
      id: "req-lock-1",
      result: {
        status: "ok",
        visibleText: expect.stringContaining("Concurrent Context"),
      },
    });
    expect(second).toMatchObject({
      jsonrpc: "2.0",
      id: "req-lock-2",
      result: {
        status: "ok",
        visibleText: expect.stringContaining("Concurrent Context"),
      },
    });
  });

  it("creates lock directories for nested persistent context paths", async () => {
    const baseDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-playwright-nested-"),
    );
    const contextDir = path.join(baseDir, "profiles", "google-search");

    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-nested-context",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <h1>Nested Context</h1>
              </main>
            </body>
          </html>
        `,
        contextDir,
        budget: 600,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-nested-context",
      result: {
        status: "ok",
        visibleText: expect.stringContaining("Nested Context"),
      },
    });
  });

  it("reuses search identity hardening across the same persistent context directory", async () => {
    const contextDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-search-profile-"),
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
      id: "req-search-profile-first",
      method: "browser.snapshot",
      params: {
        html,
        contextDir,
        searchIdentity: true,
      },
    });
    const second = await handleRequest({
      jsonrpc: "2.0",
      id: "req-search-profile-second",
      method: "browser.snapshot",
      params: {
        html,
        contextDir,
      },
    });

    expect(first).toMatchObject({
      jsonrpc: "2.0",
      id: "req-search-profile-first",
      result: {
        status: "ok",
      },
    });
    expect(second).toMatchObject({
      jsonrpc: "2.0",
      id: "req-search-profile-second",
      result: {
        status: "ok",
      },
    });
    if ("result" in second) {
      const visibleText = String(
        (second.result as { visibleText?: unknown }).visibleText ?? "",
      );
      expect(visibleText.endsWith("|object")).toBe(true);
      expect(visibleText.startsWith("true|")).toBe(false);
    }
  });

  it("executes browser-backed follow with inline html", async () => {
    const follow = await handleRequest({
      jsonrpc: "2.0",
      id: "req-follow",
      method: "browser.follow",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="step">Step 1</p>
                <a
                  href="#advanced"
                  onclick="document.getElementById('step').textContent = 'Step 2';"
                >
                  Advanced guide
                </a>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:link:advanced-guide",
        targetText: "Advanced guide",
        targetHref: "#advanced",
        headless: true,
      },
    });

    expect(follow).toMatchObject({
      jsonrpc: "2.0",
      id: "req-follow",
      result: {
        method: "browser.follow",
        limitedDynamicAction: true,
        followedRef: "rmain:link:advanced-guide",
        clickedText: "Advanced guide",
        visibleText: "Step 2 Advanced guide",
      },
    });
  });

  it("uses ordinal hints to follow the correct duplicate link", async () => {
    const follow = await handleRequest({
      jsonrpc: "2.0",
      id: "req-follow-duplicate",
      method: "browser.follow",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="step">Waiting for the correct docs link.</p>
                <section>
                  <a
                    href="#docs"
                    onclick="document.getElementById('step').textContent = 'Legacy docs opened by mistake.';"
                  >
                    Open docs
                  </a>
                </section>
                <section>
                  <a
                    href="#docs"
                    onclick="document.getElementById('step').textContent = 'Current docs opened for the research step.';"
                  >
                    Open docs
                  </a>
                </section>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:link:docs:2",
        targetText: "Open docs",
        targetHref: "#docs",
        targetTagName: "a",
        targetDomPathHint: "html > body > main > section",
        targetOrdinalHint: 2,
        headless: true,
      },
    });

    expect(follow).toMatchObject({
      jsonrpc: "2.0",
      id: "req-follow-duplicate",
      result: {
        method: "browser.follow",
        limitedDynamicAction: true,
        followedRef: "rmain:link:docs:2",
        clickedText: "Open docs",
        visibleText:
          "Current docs opened for the research step. Open docs Open docs",
      },
    });
  });

  it("ignores hidden duplicate links when resolving a follow target", async () => {
    const follow = await handleRequest({
      jsonrpc: "2.0",
      id: "req-follow-hidden-duplicate",
      method: "browser.follow",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="step">Waiting for the visible docs link.</p>
                <nav aria-hidden="true" style="display:none">
                  <a
                    href="#docs"
                    onclick="document.getElementById('step').textContent = 'Hidden docs link clicked.';"
                  >
                    Quickstart
                  </a>
                </nav>
                <section>
                  <a
                    href="#docs"
                    onclick="document.getElementById('step').textContent = 'Visible quickstart opened.';"
                  >
                    Quickstart
                  </a>
                </section>
              </main>
            </body>
          </html>
        `,
        targetRef: "rnav:link:quickstart",
        targetText: "Quickstart",
        targetHref: "#docs",
        targetTagName: "a",
        headless: true,
      },
    });

    expect(follow).toMatchObject({
      jsonrpc: "2.0",
      id: "req-follow-hidden-duplicate",
      result: {
        method: "browser.follow",
        limitedDynamicAction: true,
        followedRef: "rnav:link:quickstart",
        clickedText: "Quickstart",
        visibleText: "Visible quickstart opened. Quickstart",
      },
    });
  });

  it("falls back to safe href navigation when no visible follow target exists", async () => {
    const follow = await handleRequest({
      jsonrpc: "2.0",
      id: "req-follow-href-fallback",
      method: "browser.follow",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <nav aria-hidden="true" style="display:none">
                  <a href="#quickstart">Quickstart</a>
                </nav>
                <section id="quickstart">
                  <h1>Quickstart</h1>
                  <p>Fallback navigation reached the quickstart section.</p>
                </section>
              </main>
            </body>
          </html>
        `,
        targetRef: "rnav:link:quickstart",
        targetText: "Quickstart",
        targetHref: "#quickstart",
        targetTagName: "a",
        headless: true,
      },
    });

    expect(follow).toMatchObject({
      jsonrpc: "2.0",
      id: "req-follow-href-fallback",
      result: {
        method: "browser.follow",
        limitedDynamicAction: true,
        followedRef: "rnav:link:quickstart",
        clickedText: "Quickstart",
        finalUrl: "about:blank#quickstart",
        visibleText: expect.stringContaining(
          "Fallback navigation reached the quickstart section.",
        ),
      },
    });
  });

  it("executes browser-backed pagination with inline html", async () => {
    const paginate = await handleRequest({
      jsonrpc: "2.0",
      id: "req-3",
      method: "browser.paginate",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="page-label">Page 1</p>
                <button
                  data-direction="next"
                  onclick="document.getElementById('page-label').textContent = 'Page 2'; this.remove();"
                >
                  Next
                </button>
              </main>
            </body>
          </html>
        `,
        direction: "next",
        currentPage: 2,
        headless: true,
      },
    });

    expect(paginate).toMatchObject({
      jsonrpc: "2.0",
      id: "req-3",
      result: {
        method: "browser.paginate",
        limitedDynamicAction: true,
        page: 3,
        clickedText: "Next",
        visibleText: "Page 2",
      },
    });
  });

  it("executes browser-backed click with inline html", async () => {
    const click = await handleRequest({
      jsonrpc: "2.0",
      id: "req-click",
      method: "browser.click",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="status">Pending click.</p>
                <button
                  type="button"
                  onclick="document.getElementById('status').textContent = 'Interactive click completed.';"
                >
                  Continue
                </button>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:button:continue",
        targetText: "Continue",
        targetTagName: "button",
        headless: true,
      },
    });

    expect(click).toMatchObject({
      jsonrpc: "2.0",
      id: "req-click",
      result: {
        method: "browser.click",
        limitedDynamicAction: false,
        clickedRef: "rmain:button:continue",
        clickedText: "Continue",
        visibleText: "Interactive click completed. Continue",
      },
    });
  });

  it("executes browser-backed typing with inline html", async () => {
    const typed = await handleRequest({
      jsonrpc: "2.0",
      id: "req-type",
      method: "browser.type",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <label>
                  Email
                  <input
                    name="email"
                    type="email"
                    placeholder="name@example.com"
                    oninput="document.getElementById('status').textContent = this.value;"
                  />
                </label>
                <p id="status">Awaiting input.</p>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:input:email",
        targetText: "email email name@example.com",
        targetTagName: "input",
        targetName: "email",
        targetInputType: "email",
        value: "agent@example.com",
        headless: true,
      },
    });

    expect(typed).toMatchObject({
      jsonrpc: "2.0",
      id: "req-type",
      result: {
        method: "browser.type",
        limitedDynamicAction: false,
        typedRef: "rmain:input:email",
        typedLength: "agent@example.com".length,
        visibleText: expect.stringContaining("agent@example.com"),
      },
    });
  });

  it("executes browser-backed submit with inline html", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-submit",
      method: "browser.submit",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="status">Waiting for submission.</p>
                <form
                  onsubmit="event.preventDefault(); document.getElementById('status').textContent = 'Submitted review-ready form.';"
                >
                  <input type="email" name="email" placeholder="agent@example.com" />
                  <button type="submit">Sign in</button>
                </form>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:form:sign-in",
        targetText: "Sign in",
        targetTagName: "form",
        targetDomPathHint: "html > body > main",
        headless: true,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-submit",
      result: {
        method: "browser.submit",
        limitedDynamicAction: false,
        submittedRef: "rmain:form:sign-in",
        visibleText: "Submitted review-ready form. Sign in",
      },
    });
  });

  it("replays non-sensitive typing before submit in the same browser pass", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-submit-prefill",
      method: "browser.submit",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="status">Waiting for submission.</p>
                <form
                  onsubmit="event.preventDefault(); document.getElementById('status').textContent = document.querySelector('input[name=email]').value;"
                >
                  <label>
                    Email
                    <input type="email" name="email" placeholder="agent@example.com" />
                  </label>
                  <button type="submit">Search</button>
                </form>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:form:search-form",
        targetTagName: "form",
        targetDomPathHint: "html > body > main",
        prefill: [
          {
            targetRef: "rmain:input:email",
            targetText: "email email agent@example.com",
            targetTagName: "input",
            targetDomPathHint: "html > body > main > form > label",
            targetName: "email",
            targetInputType: "email",
            value: "agent@example.com",
          },
        ],
        headless: true,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-submit-prefill",
      result: {
        method: "browser.submit",
        visibleText: "agent@example.com Email Search",
      },
    });
  });

  it("executes browser-backed expand with inline html", async () => {
    const expand = await handleRequest({
      jsonrpc: "2.0",
      id: "req-4",
      method: "browser.expand",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <button
                  onclick="document.getElementById('details').removeAttribute('hidden');"
                >
                  Expand details
                </button>
                <p id="details" hidden>Expanded content is visible.</p>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:button:expand-pricing-details",
        targetText: "Expand details",
        headless: true,
      },
    });

    expect(expand).toMatchObject({
      jsonrpc: "2.0",
      id: "req-4",
      result: {
        method: "browser.expand",
        limitedDynamicAction: true,
        expandedRef: "rmain:button:expand-pricing-details",
        targetText: "Expand details",
        clickedText: "Expand details",
        visibleText: "Expand details Expanded content is visible.",
      },
    });
  });

  it("uses ordinal hints to expand the correct duplicate control", async () => {
    const expand = await handleRequest({
      jsonrpc: "2.0",
      id: "req-expand-duplicate",
      method: "browser.expand",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="details">No details selected yet.</p>
                <section>
                  <button
                    onclick="document.getElementById('details').textContent = 'Legacy details expanded.';"
                  >
                    Expand details
                  </button>
                </section>
                <section>
                  <button
                    onclick="document.getElementById('details').textContent = 'Current details expanded for the research flow.';"
                  >
                    Expand details
                  </button>
                </section>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:button:expand-details:2",
        targetText: "Expand details",
        targetTagName: "button",
        targetDomPathHint: "html > body > main > section",
        targetOrdinalHint: 2,
        headless: true,
      },
    });

    expect(expand).toMatchObject({
      jsonrpc: "2.0",
      id: "req-expand-duplicate",
      result: {
        method: "browser.expand",
        limitedDynamicAction: true,
        expandedRef: "rmain:button:expand-details:2",
        clickedText: "Expand details",
        visibleText:
          "Current details expanded for the research flow. Expand details Expand details",
      },
    });
  });

  it("rejects malformed snapshot requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-5",
      method: "browser.snapshot",
      params: {},
    });

    expect(response).toEqual({
      jsonrpc: "2.0",
      id: "req-5",
      error: {
        code: -32602,
        message:
          "browser.snapshot requires `params.url`, `params.html`, or `params.contextDir`.",
      },
    });
  });

  it("rejects malformed paginate requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-6",
      method: "browser.paginate",
      params: {
        direction: "next",
      },
    });

    expect(response).toEqual({
      jsonrpc: "2.0",
      id: "req-6",
      error: {
        code: -32602,
        message:
          "browser.paginate requires either `params.url` or `params.html`.",
      },
    });
  });

  it("rejects malformed follow requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-7",
      method: "browser.follow",
      params: {
        targetRef: "rmain:link:advanced-guide",
      },
    });

    expect(response).toEqual({
      jsonrpc: "2.0",
      id: "req-7",
      error: {
        code: -32602,
        message:
          "browser.follow requires either `params.url` or `params.html`.",
      },
    });
  });
});
