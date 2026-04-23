import type { BrowserContext, Frame, Page } from "playwright";

import { readProbeFallback } from "./error-tolerance.js";
import { normalizeWhitespace } from "./shared.js";
import type { TargetDescriptor } from "./types.js";

type ClosedShadowSnapshot = {
  readonly hostPath: string;
  readonly html: string;
  readonly text: string;
  readonly linkCount: number;
  readonly buttonCount: number;
  readonly inputCount: number;
  readonly links: Array<{ text: string; href: string | null }>;
};

type ClosedShadowActionPayload = {
  readonly kind: "click" | "follow" | "type" | "submit";
  readonly target: TargetDescriptor;
  readonly value?: string;
  readonly prefill?: Array<{
    readonly value: string;
    readonly target: TargetDescriptor;
  }>;
};

type ClosedShadowActionResult = {
  readonly targetText: string;
  readonly targetHref?: string;
  readonly clickedText?: string;
  readonly typedLength?: number;
};

type EvaluateTarget = Page | Frame;

const DOM_INSTRUMENTATION_SCRIPT = String.raw`
(() => {
  if (globalThis.__touchBrowserDomInstrumentationInstalled) {
    return;
  }
  const registry = [];
  const normalize = (value) => String(value || "").replace(/\s+/g, " ").trim();
  const originalAttachShadow = Element.prototype.attachShadow;
  const elementPath = (element) => {
    const parts = [];
    let current = element;
    while (current && current.tagName) {
      parts.unshift(current.tagName.toLowerCase());
      const root = current.getRootNode ? current.getRootNode() : null;
      current = current.parentElement || (root && root.host ? root.host : null);
    }
    return parts.join(" > ");
  };
  const isVisible = (element) => {
    try {
      const ownerWindow = element.ownerDocument && element.ownerDocument.defaultView
        ? element.ownerDocument.defaultView
        : window;
      const style = ownerWindow.getComputedStyle(element);
      const rect = typeof element.getBoundingClientRect === "function"
        ? element.getBoundingClientRect()
        : { width: 1, height: 1 };
      return style.visibility !== "hidden" && style.display !== "none" && rect.width >= 0 && rect.height >= 0;
    } catch {
      return true;
    }
  };
  const describe = (element, index) => {
    if (!isVisible(element)) return null;
    const tagName = element.tagName.toLowerCase();
    const href = element.getAttribute("href") || undefined;
    const name = element.getAttribute("name") || "";
    const inputType = element.getAttribute("type") || "";
    const placeholder = element.getAttribute("placeholder") || "";
    const ariaLabel = element.getAttribute("aria-label") || "";
    const value = "value" in element ? element.value || "" : "";
    const text = normalize(element.innerText || element.textContent || [name, inputType, placeholder, value, ariaLabel].join(" "));
    if (!text && !href) return null;
    const fullPath = elementPath(element);
    const parent = element.parentElement || (element.getRootNode && element.getRootNode().host) || null;
    const parentPath = parent ? elementPath(parent) : "";
    return {
      element,
      index,
      text,
      href,
      tagName,
      fullPath,
      parentPath,
    };
  };
  const scoreTextMatch = (candidateText, targetText) => {
    if (!targetText) return 0;
    if (candidateText === targetText) return 5;
    if (candidateText.includes(targetText) || targetText.includes(candidateText)) return 3;
    return undefined;
  };
  const scoreHrefMatch = (candidateHref, targetHref) => {
    if (!targetHref) return 0;
    if (candidateHref === targetHref) return 4;
    return candidateHref ? undefined : 0;
  };
  const scoreTagNameMatch = (candidateTagName, targetTagName) => {
    if (!targetTagName) return 0;
    return candidateTagName === String(targetTagName).toLowerCase() ? 2 : undefined;
  };
  const scoreContainsMatch = (candidateText, targetValue, score) => {
    if (!targetValue) return 0;
    return candidateText.includes(String(targetValue).toLowerCase()) ? score : undefined;
  };
  const scoreDomPathMatch = (candidate, domPathHint) => {
    if (!domPathHint) return 0;
    const normalizedHint = String(domPathHint).toLowerCase();
    if (candidate.parentPath === normalizedHint) return 6;
    if (candidate.fullPath === normalizedHint) return 5;
    return candidate.fullPath.startsWith(normalizedHint + " >") ? 2 : 0;
  };
  const scoreCandidate = (candidate, target) => {
    const candidateText = candidate.text.toLowerCase();
    const targetText = normalize(target.text || "").toLowerCase();
    const partialScores = [
      scoreTextMatch(candidateText, targetText),
      scoreHrefMatch(candidate.href, target.href),
      scoreTagNameMatch(candidate.tagName, target.tagName),
      scoreContainsMatch(candidateText, target.name, 2),
      scoreContainsMatch(candidateText, target.inputType, 1),
      scoreDomPathMatch(candidate, target.domPathHint),
    ];
    let score = 0;
    for (const partialScore of partialScores) {
      if (partialScore === undefined) return 0;
      score += partialScore;
    }
    return score;
  };
  const queryClosedRoots = () => registry
    .filter((entry) => entry.host && entry.root)
    .map((entry) => ({ host: entry.host, root: entry.root }));
  const findBest = (selector, target) => {
    const candidates = [];
    for (const entry of queryClosedRoots()) {
      const elements = Array.from(entry.root.querySelectorAll(selector));
      for (const [index, element] of elements.entries()) {
        const described = describe(element, index);
        if (!described) continue;
        const score = scoreCandidate(described, target || {});
        if (score > 0) candidates.push({ ...described, score });
      }
    }
    candidates.sort((left, right) => {
      const scoreDiff = right.score - left.score;
      if (scoreDiff !== 0) return scoreDiff;
      return left.index - right.index;
    });
    if (candidates.length === 0) return null;
    const ordinalHint = Number(target && target.ordinalHint ? target.ordinalHint : 0);
    if (ordinalHint > 1) {
      const topScore = candidates[0].score;
      const topCandidates = candidates.filter((candidate) => candidate.score === topScore);
      return topCandidates[ordinalHint - 1] || candidates[ordinalHint - 1] || candidates[0];
    }
    return candidates[0];
  };
  const fill = (element, value) => {
    const tagName = element.tagName.toLowerCase();
    if (tagName === "input" || tagName === "textarea") {
      element.focus();
      element.value = value;
    } else if (element.hasAttribute("contenteditable")) {
      element.focus();
      element.textContent = value;
    } else {
      throw new Error("Target input does not support typing.");
    }
    element.dispatchEvent(new Event("input", { bubbles: true }));
    element.dispatchEvent(new Event("change", { bubbles: true }));
  };
  const click = (element) => {
    element.scrollIntoView({ block: "center", inline: "center" });
    element.click();
  };
  globalThis.__touchBrowserCollectClosedShadowRoots = () =>
    queryClosedRoots().map((entry) => {
      const links = Array.from(entry.root.querySelectorAll("a")).slice(0, 50).map((element) => ({
        text: normalize(element.innerText || element.textContent || ""),
        href: element.getAttribute("href"),
      }));
      return {
        hostPath: elementPath(entry.host),
        html: entry.root.innerHTML || "",
        text: normalize(entry.root.textContent || ""),
        linkCount: entry.root.querySelectorAll("a").length,
        buttonCount: entry.root.querySelectorAll("button").length,
        inputCount: entry.root.querySelectorAll("input").length,
        links,
      };
    }).filter((entry) => entry.text || entry.html);
  globalThis.__touchBrowserPerformClosedShadowAction = (payload) => {
    const target = payload && payload.target ? payload.target : {};
    const kind = payload && payload.kind ? payload.kind : "click";
    if (kind === "type") {
      const candidate = findBest("input, textarea, [contenteditable='true']", target);
      if (!candidate) return null;
      fill(candidate.element, String(payload.value || ""));
      return { targetText: candidate.text, typedLength: String(payload.value || "").length };
    }
    if (kind === "submit") {
      for (const entry of payload.prefill || []) {
        const candidate = findBest("input, textarea, [contenteditable='true']", entry.target || {});
        if (candidate) {
          fill(candidate.element, String(entry.value || ""));
        }
      }
      const candidate = findBest("form, button[type='submit'], input[type='submit'], button, input[type='button']", target);
      if (!candidate) return null;
      if (candidate.element.tagName.toLowerCase() === "form") {
        if (typeof candidate.element.requestSubmit === "function") candidate.element.requestSubmit();
        else candidate.element.submit();
      } else {
        click(candidate.element);
      }
      return { targetText: candidate.text, clickedText: candidate.text };
    }
    const selector = kind === "follow"
      ? "a"
      : "button, [role='button'], a, input[type='submit'], input[type='button'], input[type='checkbox'], input[type='radio']";
    const candidate = findBest(selector, target);
    if (!candidate) return null;
    click(candidate.element);
    return {
      targetText: candidate.text,
      targetHref: candidate.href,
      clickedText: candidate.text,
    };
  };
  Element.prototype.attachShadow = function(init) {
    const root = originalAttachShadow.call(this, init);
    if (init && init.mode === "closed") {
      registry.push({ host: this, root });
    }
    return root;
  };
  globalThis.__touchBrowserDomInstrumentationInstalled = true;
})();
`;

export async function installDomInstrumentation(
  context: BrowserContext,
): Promise<void> {
  await context.addInitScript({ content: DOM_INSTRUMENTATION_SCRIPT });
}

export async function collectClosedShadowSnapshots(
  target: EvaluateTarget,
): Promise<ClosedShadowSnapshot[]> {
  return readProbeFallback(
    target.evaluate(() => {
      const collector = (
        globalThis as typeof globalThis & {
          __touchBrowserCollectClosedShadowRoots?: () => ClosedShadowSnapshot[];
        }
      ).__touchBrowserCollectClosedShadowRoots;
      return typeof collector === "function" ? collector() : [];
    }),
    [],
    "collectClosedShadowSnapshots",
  );
}

export async function performClosedShadowActionAcrossFrames(
  page: Page,
  payload: ClosedShadowActionPayload,
): Promise<ClosedShadowActionResult | null> {
  const frames = [
    page.mainFrame(),
    ...page.frames().filter((frame) => frame !== page.mainFrame()),
  ];
  for (const frame of frames) {
    const result = await performClosedShadowAction(frame, payload);
    if (result) {
      return result;
    }
  }

  return null;
}

async function performClosedShadowAction(
  target: EvaluateTarget,
  payload: ClosedShadowActionPayload,
): Promise<ClosedShadowActionResult | null> {
  return readProbeFallback(
    target.evaluate((actionPayload) => {
      const performer = (
        globalThis as typeof globalThis & {
          __touchBrowserPerformClosedShadowAction?: (
            payload: ClosedShadowActionPayload,
          ) => ClosedShadowActionResult | null;
        }
      ).__touchBrowserPerformClosedShadowAction;
      return typeof performer === "function" ? performer(actionPayload) : null;
    }, payload),
    null,
    "performClosedShadowAction",
  );
}

export async function captureFrameLinks(
  frame: Frame,
  maxLinks: number,
): Promise<Array<{ text: string; href: string | null }>> {
  const linkCount = await readProbeFallback(
    frame.locator("a").count(),
    0,
    "captureFrameLinks count",
  );
  const links = [];

  for (let index = 0; index < Math.min(linkCount, maxLinks); index += 1) {
    const locator = frame.locator("a").nth(index);
    const [text, href] = await Promise.all([
      readProbeFallback(
        locator.textContent(),
        "",
        `captureFrameLinks text ${index}`,
      ),
      readProbeFallback(
        locator.getAttribute("href"),
        null,
        `captureFrameLinks href ${index}`,
      ),
    ]);
    links.push({
      text: normalizeWhitespace(text ?? ""),
      href,
    });
  }

  return links;
}
