import { mkdtemp } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { handleRequest, readVisibleText } from "../support/adapter-helpers.js";

describe("playwright adapter browser search and context extensions", () => {
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

  it("supports profileDir for embedded persistent browser sessions", async () => {
    const baseDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-playwright-profile-"),
    );
    const profileDir = path.join(baseDir, "profiles", "shared-search");

    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-profile-dir",
      method: "browser.snapshot",
      params: {
        html: `
          <!doctype html>
          <html>
            <body>
              <main>
                <h1>Shared Profile</h1>
              </main>
            </body>
          </html>
        `,
        profileDir,
        budget: 600,
        headless: true,
      },
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-profile-dir",
      result: {
        status: "ok",
        visibleText: expect.stringContaining("Shared Profile"),
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

    const visibleText = readVisibleText(second);
    expect(visibleText.endsWith("|object")).toBe(true);
    expect(visibleText.startsWith("true|")).toBe(false);
  });
});
