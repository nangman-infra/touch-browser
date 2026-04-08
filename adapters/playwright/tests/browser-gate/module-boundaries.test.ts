import { mkdtemp, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import { describe, expect, it } from "vitest";

import {
  fillTargetLocator,
  nextPaginationSelectors,
  prevPaginationSelectors,
  resolveSafeFollowUrl,
  settleAfterAction,
  submitTargetLocator,
} from "../../src/action-helpers.js";
import {
  browserSource,
  capturePageState,
  withPage,
} from "../../src/browser-runtime.js";
import {
  findClickLocator,
  findExpandLocator,
  findFirstLocator,
  findFollowLocator,
  findSubmitLocator,
  findTypeLocator,
} from "../../src/locator-scoring.js";

describe("playwright adapter module boundaries", () => {
  it("captures browser page state from inline HTML", async () => {
    const state = await withPage(
      browserSource(
        undefined,
        `
          <!doctype html>
          <html>
            <head><title>Module State</title></head>
            <body>
              <main>
                <h1>Module State</h1>
                <p>Visible text should be normalized.</p>
                <a href="/docs/start">Start here</a>
                <button type="button">Continue</button>
                <input type="email" name="email" value="person@example.com" />
              </main>
            </body>
          </html>
        `,
        true,
        undefined,
        undefined,
        false,
      ),
      async (page) => capturePageState(page),
    );

    expect(state).toMatchObject({
      finalUrl: "about:blank",
      title: "Module State",
      visibleText:
        "Module State Visible text should be normalized. Start here Continue",
      linkCount: 1,
      buttonCount: 1,
      inputCount: 1,
      links: [{ text: "Start here", href: "/docs/start" }],
    });
    expect(state.html).toContain("Visible text should be normalized.");
    expect(state.htmlLength).toBeGreaterThan(50);
  });

  it("supports persistent context execution and releases the lock file", async () => {
    const contextDir = await mkdtemp(
      path.join(os.tmpdir(), "touch-browser-adapter-context-"),
    );
    const lockDir = `${contextDir}.touch-browser-lock`;

    try {
      const title = await withPage(
        browserSource(
          undefined,
          `
            <!doctype html>
            <html>
              <head><title>Persistent Context</title></head>
              <body><main><h1>Persistent Context</h1></main></body>
            </html>
          `,
          true,
          contextDir,
          undefined,
          false,
        ),
        async (page) => page.title(),
      );

      expect(title).toBe("Persistent Context");
      await expect(stat(lockDir)).rejects.toMatchObject({ code: "ENOENT" });
    } finally {
      await rm(lockDir, { recursive: true, force: true }).catch(() => {});
      await rm(contextDir, { recursive: true, force: true });
    }
  });

  it("tests action helpers and locator scoring independently", async () => {
    await withPage(
      browserSource(
        undefined,
        `
          <!doctype html>
          <html>
            <body>
              <main>
                <nav>
                  <a href="/guide/one">Read more</a>
                  <a href="/guide/two">Read more</a>
                  <a rel="next" href="/guide/next">Next page</a>
                </nav>
                <section>
                  <button id="cta" type="button">Continue</button>
                  <button
                    id="expand-button"
                    type="button"
                    aria-expanded="false"
                  >
                    Show advanced options
                  </button>
                  <form id="signup-form">
                    <input
                      id="email-input"
                      type="email"
                      name="email"
                      placeholder="Email address"
                    />
                    <button id="submit-button" type="submit">
                      Submit form
                    </button>
                  </form>
                  <div id="editor" contenteditable="true"></div>
                  <button id="plain-button" type="button">Plain button</button>
                </section>
              </main>
            </body>
          </html>
        `,
        true,
        undefined,
        undefined,
        false,
      ),
      async (page) => {
        const inputLocator = page.locator("#email-input");
        await fillTargetLocator(page, inputLocator, "person@example.com");
        expect(await inputLocator.inputValue()).toBe("person@example.com");

        const editorLocator = page.locator("#editor");
        await fillTargetLocator(page, editorLocator, "Typed note");
        expect(await editorLocator.textContent()).toContain("Typed note");

        await expect(
          fillTargetLocator(page, page.locator("#plain-button"), "nope"),
        ).rejects.toThrow("Target input does not support typing.");

        const followLocator = await findFollowLocator(page, {
          text: "Read more",
          href: undefined,
          tagName: "a",
          domPathHint: undefined,
          ordinalHint: 2,
          name: undefined,
          inputType: undefined,
        });
        expect(await followLocator?.getAttribute("href")).toBe("/guide/two");

        const clickLocator = await findClickLocator(page, {
          text: "Continue",
          href: undefined,
          tagName: "button",
          domPathHint: undefined,
          ordinalHint: undefined,
          name: undefined,
          inputType: undefined,
        });
        expect(await clickLocator?.getAttribute("id")).toBe("cta");

        const typeLocator = await findTypeLocator(page, {
          text: "Email address",
          href: undefined,
          tagName: "input",
          domPathHint: undefined,
          ordinalHint: undefined,
          name: "email",
          inputType: "email",
        });
        expect(await typeLocator?.getAttribute("id")).toBe("email-input");

        const submitLocator = await findSubmitLocator(page, {
          text: "Submit form",
          href: undefined,
          tagName: "button",
          domPathHint: undefined,
          ordinalHint: undefined,
          name: undefined,
          inputType: undefined,
        });
        expect(await submitLocator?.getAttribute("id")).toBe("submit-button");

        const expandLocator = await findExpandLocator(page, {
          text: "Show advanced options",
          href: undefined,
          tagName: "button",
          domPathHint: undefined,
          ordinalHint: undefined,
          name: undefined,
          inputType: undefined,
        });
        expect(await expandLocator?.getAttribute("id")).toBe("expand-button");

        const firstLocator = await findFirstLocator(page, [
          "[data-missing='true']",
          "a[rel='next']",
        ]);
        expect(await firstLocator?.getAttribute("href")).toBe("/guide/next");

        await page.evaluate(() => {
          const body = document.body;
          body.setAttribute("data-submit-count", "0");

          const form = document.getElementById("signup-form");
          form?.addEventListener(
            "submit",
            (event) => {
              event.preventDefault();
              body.setAttribute("data-submit-count", "1");
            },
            { once: true },
          );

          const button = document.getElementById("cta");
          button?.addEventListener(
            "click",
            () => {
              body.setAttribute("data-clicked", "yes");
            },
            { once: true },
          );
        });

        await submitTargetLocator(page.locator("#signup-form"));
        expect(
          await page.locator("body").getAttribute("data-submit-count"),
        ).toBe("1");

        await submitTargetLocator(page.locator("#cta"));
        expect(await page.locator("body").getAttribute("data-clicked")).toBe(
          "yes",
        );

        await settleAfterAction(page);
      },
    );

    expect(
      resolveSafeFollowUrl("https://example.com/docs/start", "../guide"),
    ).toBe("https://example.com/guide");
    expect(
      resolveSafeFollowUrl(
        "https://example.com/docs/start",
        "https://other.example.com/guide",
      ),
    ).toBeUndefined();
    expect(nextPaginationSelectors()).toContain("a[rel='next']");
    expect(prevPaginationSelectors()).toContain("a[rel='prev']");
  }, 15_000);
});
