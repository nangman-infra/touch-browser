import { describe, expect, it } from "vitest";

import {
  extractFirstElementInnerHtml,
  replaceElementBlocks,
  replaceTags,
  stripHtmlPreservingMarkdown,
} from "../../../../scripts/lib/html-utils.mjs";
import {
  normalizeCleanedDom,
  stripHtml,
} from "../../../../scripts/lib/live-sample-server.mjs";

describe("html utils", () => {
  it("strips raw-text blocks and keeps only safe attributes in normalized dom", () => {
    const html = `<!doctype html>
      <main>
        <!-- hidden -->
        <style>.secret { display: none; }</style>
        <script>window.secret = true;</script>
        <a href="/docs" aria-label="Docs" onclick="alert('x')">Docs</a>
        <img src="/hero.png" data-testid="hero" alt="hero" />
      </main>`;

    const visibleText = stripHtml(html);
    const cleanedDom = normalizeCleanedDom(html);

    expect(visibleText).toContain("Docs");
    expect(visibleText).not.toContain("window.secret");
    expect(visibleText).not.toContain(".secret");
    expect(cleanedDom).toContain('<a href="/docs" aria-label="Docs">');
    expect(cleanedDom).toContain('<img src="/hero.png">');
    expect(cleanedDom).not.toContain("onclick");
    expect(cleanedDom).not.toContain("data-testid");
  });

  it("prefers the first semantic container when extracting main content", () => {
    const html =
      "<html><body><article>fallback</article><main><p>primary</p></main></body></html>";

    expect(
      extractFirstElementInnerHtml(html, ["main", "article", "body"]),
    ).toBe("<p>primary</p>");
  });

  it("supports markdown-oriented replacements without HTML regex parsing", () => {
    let mainHtml = `<main>
      <h2>Docs</h2>
      <p>Hello<br />world</p>
      <a href="/docs/getting-started">Read more</a>
      <ul><li>First step</li></ul>
    </main>`;

    mainHtml = replaceTags(mainHtml, ["br"], () => "\n");
    mainHtml = replaceTags(
      mainHtml,
      ["p", "div", "section", "article", "main", "ul", "ol"],
      ({ closing }) => (closing ? "\n" : ""),
    );
    mainHtml = replaceElementBlocks(mainHtml, "h2", ({ inner }) => {
      const text = stripHtml(inner).trim();
      return text ? `\n## ${text}\n` : "\n";
    });
    mainHtml = replaceElementBlocks(mainHtml, "a", ({ attributes, inner }) => {
      const href = attributes.find(({ name }) => name === "href")?.value;
      const text = stripHtml(inner).trim();
      return href && text ? `[${text}](${href})` : text;
    });
    mainHtml = replaceElementBlocks(mainHtml, "li", ({ inner }) => {
      const text = stripHtml(inner).trim();
      return text ? `\n- ${text}\n` : "\n";
    });

    const markdown = stripHtmlPreservingMarkdown(mainHtml)
      .split("\n")
      .map((line) => line.trim())
      .filter(Boolean)
      .join("\n");

    expect(markdown).toContain("## Docs");
    expect(markdown).toContain("Hello");
    expect(markdown).toContain("world");
    expect(markdown).toContain("[Read more](/docs/getting-started)");
    expect(markdown).toContain("- First step");
    expect(markdown).not.toContain("<a ");
  });
});
