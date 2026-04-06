import { describe, expect, it } from "vitest";

import {
  handleRequest,
  expectJsonRpcSuccess,
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
});
