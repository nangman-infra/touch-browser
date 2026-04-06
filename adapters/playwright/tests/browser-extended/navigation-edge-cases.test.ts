import { describe, expect, it } from "vitest";

import { handleRequest } from "../support/adapter-helpers.js";

describe("playwright adapter browser edge cases", () => {
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
});
