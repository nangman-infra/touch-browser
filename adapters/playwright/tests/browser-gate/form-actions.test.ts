import { describe, expect, it } from "vitest";

import { handleRequest } from "../support/adapter-helpers.js";

describe("playwright adapter browser form actions", () => {
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
});
