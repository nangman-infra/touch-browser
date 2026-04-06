import { describe, expect, it } from "vitest";

import { handleRequest } from "../support/adapter-helpers.js";

describe("playwright adapter browser navigation actions", () => {
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
});
