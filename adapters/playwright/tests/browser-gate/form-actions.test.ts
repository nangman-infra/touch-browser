import http from "node:http";
import { describe, expect, it } from "vitest";

import { handleRequest } from "../support/adapter-helpers.js";

describe("playwright adapter browser form actions", () => {
  it("clicks targets inside cross-origin iframes", async () => {
    const childServer = await startServer((request, response) => {
      if (request.url === "/child") {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        response.end(
          "<!doctype html><html><body><main><p id='status'>Pending frame click.</p><button type='button' onclick=\"document.getElementById('status').textContent='Frame click completed.';\">Frame Continue</button></main></body></html>",
        );
        return;
      }
      response.writeHead(404).end("missing");
    });
    const parentServer = await startServer((request, response) => {
      if (request.url === "/parent") {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        response.end(
          `<!doctype html><html><body><main><h1>Parent</h1><iframe src="${childServer.url}/child" title="frame"></iframe></main></body></html>`,
        );
        return;
      }
      response.writeHead(404).end("missing");
    });

    try {
      const click = await handleRequest({
        jsonrpc: "2.0",
        id: "req-frame-click",
        method: "browser.click",
        params: {
          url: `${parentServer.url}/parent`,
          targetRef: "rmain:button:frame-continue",
          targetText: "Frame Continue",
          targetTagName: "button",
          headless: true,
        },
      });

      expect(click).toMatchObject({
        jsonrpc: "2.0",
        id: "req-frame-click",
        result: {
          method: "browser.click",
          clickedText: "Frame Continue",
          visibleText: expect.stringContaining("Frame click completed."),
        },
      });
    } finally {
      await Promise.all([
        closeServer(parentServer.server),
        closeServer(childServer.server),
      ]);
    }
  }, 15_000);

  it("clicks closed shadow DOM targets inside cross-origin iframes", async () => {
    const childServer = await startServer((request, response) => {
      if (request.url === "/child") {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        response.end(`<!doctype html>
          <html>
            <body>
              <main>
                <p id="status">Pending nested shadow click.</p>
                <script>
                  customElements.define("nested-shadow-app", class extends HTMLElement {
                    connectedCallback() {
                      const root = this.attachShadow({ mode: "closed" });
                      root.innerHTML = "<button type='button'>Nested Shadow Continue</button><p>Nested hidden text.</p>";
                      root.querySelector("button").onclick = () => {
                        document.getElementById("status").textContent = "Nested shadow click completed.";
                      };
                    }
                  });
                </script>
                <nested-shadow-app></nested-shadow-app>
              </main>
            </body>
          </html>`);
        return;
      }
      response.writeHead(404).end("missing");
    });
    const parentServer = await startServer((request, response) => {
      if (request.url === "/parent") {
        response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
        response.end(
          `<!doctype html><html><body><main><h1>Nested Parent</h1><iframe src="${childServer.url}/child" title="nested"></iframe></main></body></html>`,
        );
        return;
      }
      response.writeHead(404).end("missing");
    });

    try {
      const click = await handleRequest({
        jsonrpc: "2.0",
        id: "req-nested-shadow-click",
        method: "browser.click",
        params: {
          url: `${parentServer.url}/parent`,
          targetRef: "rmain:button:nested-shadow-continue",
          targetText: "Nested Shadow Continue",
          targetTagName: "button",
          headless: true,
        },
      });

      expect(click).toMatchObject({
        jsonrpc: "2.0",
        id: "req-nested-shadow-click",
        result: {
          method: "browser.click",
          clickedText: "Nested Shadow Continue",
          visibleText: expect.stringContaining(
            "Nested shadow click completed.",
          ),
        },
      });
    } finally {
      await Promise.all([
        closeServer(parentServer.server),
        closeServer(childServer.server),
      ]);
    }
  }, 15_000);

  it("clicks closed shadow DOM targets", async () => {
    const click = await handleRequest({
      jsonrpc: "2.0",
      id: "req-closed-shadow-click",
      method: "browser.click",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="status">Pending click.</p>
                <closed-card></closed-card>
                <script>
                  customElements.define("closed-card", class extends HTMLElement {
                    connectedCallback() {
                      const root = this.attachShadow({ mode: "closed" });
                      const button = document.createElement("button");
                      button.type = "button";
                      button.textContent = "Shadow Continue";
                      button.onclick = () => {
                        document.getElementById("status").textContent = "Shadow click completed.";
                      };
                      root.appendChild(button);
                    }
                  });
                </script>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:button:shadow-continue",
        targetText: "Shadow Continue",
        targetTagName: "button",
        headless: true,
      },
    });

    expect(click).toMatchObject({
      jsonrpc: "2.0",
      id: "req-closed-shadow-click",
      result: {
        method: "browser.click",
        clickedText: "Shadow Continue",
        visibleText: expect.stringContaining("Shadow click completed."),
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

  it("waits for delayed SPA updates after click actions settle", async () => {
    const click = await handleRequest({
      jsonrpc: "2.0",
      id: "req-click-delayed",
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
                  onclick="setTimeout(() => { document.getElementById('status').textContent = 'Delayed click completed.'; }, 600);"
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
      id: "req-click-delayed",
      result: {
        method: "browser.click",
        visibleText: "Delayed click completed. Continue",
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

  it("submits closed shadow DOM forms", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-shadow-submit",
      method: "browser.submit",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <p id="status">Waiting for submission.</p>
                <closed-login></closed-login>
                <script>
                  customElements.define("closed-login", class extends HTMLElement {
                    connectedCallback() {
                      const root = this.attachShadow({ mode: "closed" });
                      const form = document.createElement("form");
                      form.onsubmit = (event) => {
                        event.preventDefault();
                        document.getElementById("status").textContent = "Closed shadow form submitted.";
                      };
                      form.innerHTML = "<input type='email' name='email' placeholder='agent@example.com' /><button type='submit'>Shadow Sign in</button>";
                      root.appendChild(form);
                    }
                  });
                </script>
              </main>
            </body>
          </html>
        `,
        targetRef: "rmain:form:shadow-sign-in",
        targetText: "Shadow Sign in",
        targetTagName: "form",
        headless: true,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-shadow-submit",
      result: {
        method: "browser.submit",
        visibleText: expect.stringContaining("Closed shadow form submitted."),
      },
    });
  });
});

function startServer(
  handler: http.RequestListener,
): Promise<{ server: http.Server; url: string }> {
  return new Promise((resolve) => {
    const server = http.createServer(handler);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        throw new Error("Failed to resolve test server port.");
      }
      resolve({
        server,
        url: `http://127.0.0.1:${address.port}`,
      });
    });
  });
}

function closeServer(server: http.Server): Promise<void> {
  return new Promise((resolve, reject) => {
    server.close((error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
}
