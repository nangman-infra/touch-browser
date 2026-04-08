import type { Locator, Page } from "playwright";

import { settleAfterAction } from "./action-helpers.js";
import {
  ignoreOptionalActionFailure,
  readProbeFallback,
} from "./error-tolerance.js";
import { normalizeWhitespace } from "./shared.js";
import {
  EVIDENCE_SELECTOR_KEYWORDS,
  type EvidencePopupSnapshot,
  type EvidenceSelectorCandidate,
  type EvidenceSelectorTarget,
  MAX_EVIDENCE_SELECTOR_CANDIDATES,
} from "./types.js";

export async function maybeExpandEvidenceSelectors(page: Page): Promise<void> {
  const seenControls = new Set<string>();
  const popupSnapshots: EvidencePopupSnapshot[] = [];
  const selectors = [
    "button[aria-haspopup='listbox'][aria-expanded='false']",
    "[role='combobox'][aria-expanded='false']",
  ];

  for (const selector of selectors) {
    popupSnapshots.push(
      ...(await collectEvidencePopupSnapshots(page, selector, seenControls)),
    );
  }

  await injectEvidencePopupSnapshots(page, popupSnapshots);
}

async function collectEvidencePopupSnapshots(
  page: Page,
  selector: string,
  seenControls: Set<string>,
): Promise<EvidencePopupSnapshot[]> {
  const popupSnapshots: EvidencePopupSnapshot[] = [];

  for (
    let processed = 0;
    processed < MAX_EVIDENCE_SELECTOR_CANDIDATES;
    processed += 1
  ) {
    const target = await nextEvidenceSelectorTarget(
      page,
      selector,
      seenControls,
    );
    if (!target) {
      break;
    }

    const popupSnapshot = await openEvidenceSelectorAndCapturePopup(
      page,
      target,
    );
    if (popupSnapshot) {
      popupSnapshots.push(popupSnapshot);
    }
  }

  return popupSnapshots;
}

async function nextEvidenceSelectorTarget(
  page: Page,
  selector: string,
  seenControls: Set<string>,
): Promise<EvidenceSelectorTarget | undefined> {
  const candidates = await describeEvidenceSelectorCandidates(page, selector);

  for (const candidate of candidates) {
    if (
      !candidate.descriptor ||
      !looksLikeEvidenceSelector(candidate.descriptor)
    ) {
      continue;
    }
    if (seenControls.has(candidate.cacheKey) || !candidate.popupId) {
      continue;
    }

    const locator = page.locator(selector).nth(candidate.index);
    if (!(await isLocatorVisible(locator))) {
      continue;
    }

    seenControls.add(candidate.cacheKey);
    return {
      locator,
      popupId: candidate.popupId,
      descriptor: candidate.descriptor,
    };
  }

  return undefined;
}

async function describeEvidenceSelectorCandidates(
  page: Page,
  selector: string,
): Promise<readonly EvidenceSelectorCandidate[]> {
  const candidates: EvidenceSelectorCandidate[] = [];
  const selectorLocator = page.locator(selector);
  const candidateCount = await readProbeFallback(
    selectorLocator.count(),
    0,
    `describeEvidenceSelectorCandidates count ${selector}`,
  );

  for (let index = 0; index < candidateCount; index += 1) {
    const locator = selectorLocator.nth(index);
    const descriptor = await selectorDescriptor(locator);
    candidates.push({
      index,
      descriptor,
      cacheKey: descriptor.toLowerCase(),
      popupId: await readProbeFallback(
        locator.getAttribute("aria-controls"),
        null,
        `describeEvidenceSelectorCandidates popupId ${selector}:${index}`,
      ),
    });
  }

  return candidates;
}

async function isLocatorVisible(locator: Locator): Promise<boolean> {
  return await readProbeFallback(
    locator.isVisible(),
    false,
    "isLocatorVisible",
  );
}

async function openEvidenceSelectorAndCapturePopup(
  page: Page,
  target: EvidenceSelectorTarget,
): Promise<EvidencePopupSnapshot | undefined> {
  await ignoreOptionalActionFailure(
    target.locator.click(),
    `openEvidenceSelectorAndCapturePopup click ${target.descriptor}`,
  );
  await settleAfterAction(page);
  const popupSnapshot = await captureEvidencePopupSnapshot(
    page,
    target.popupId,
    target.descriptor,
  );
  await closeEvidencePopup(page, target.popupId);
  return popupSnapshot;
}

async function captureEvidencePopupSnapshot(
  page: Page,
  popupId: string,
  descriptor: string,
): Promise<EvidencePopupSnapshot | undefined> {
  const popupHtml = await readProbeFallback(
    popupLocator(page, popupId).evaluate((popup) => {
      if (!(popup instanceof HTMLElement)) {
        return undefined;
      }

      const clone = popup.cloneNode(true);
      if (!(clone instanceof HTMLElement)) {
        return undefined;
      }

      for (const styleNode of clone.querySelectorAll("style")) {
        styleNode.remove();
      }
      return clone.outerHTML;
    }),
    undefined,
    `captureEvidencePopupSnapshot ${popupId}`,
  );

  if (!popupHtml) {
    return undefined;
  }

  return {
    id: `touch-browser-evidence-popup-${popupId}`,
    label: descriptor,
    html: popupHtml,
  };
}

async function closeEvidencePopup(page: Page, popupId: string): Promise<void> {
  await ignoreOptionalActionFailure(
    page.keyboard.press("Escape"),
    `closeEvidencePopup escape ${popupId}`,
  );
  await ignoreOptionalActionFailure(
    page.waitForTimeout(100),
    `closeEvidencePopup wait after escape ${popupId}`,
  );
  const popup = popupLocator(page, popupId);
  const [popupStillVisible, hiddenAttribute, ariaHidden, dataState] =
    await Promise.all([
      readProbeFallback(
        popup.isVisible(),
        false,
        `closeEvidencePopup visible ${popupId}`,
      ),
      readProbeFallback(
        popup.getAttribute("hidden"),
        null,
        `closeEvidencePopup hidden ${popupId}`,
      ),
      readProbeFallback(
        popup.getAttribute("aria-hidden"),
        null,
        `closeEvidencePopup aria-hidden ${popupId}`,
      ),
      readProbeFallback(
        popup.evaluate((node) =>
          node instanceof HTMLElement ? (node.dataset.state ?? null) : null,
        ),
        null,
        `closeEvidencePopup data-state ${popupId}`,
      ),
    ]);
  const popupStillOpen =
    popupStillVisible &&
    hiddenAttribute === null &&
    ariaHidden !== "true" &&
    dataState !== "closed";

  if (popupStillOpen) {
    await ignoreOptionalActionFailure(
      page.mouse.click(8, 8),
      `closeEvidencePopup outside click ${popupId}`,
    );
    await ignoreOptionalActionFailure(
      page.waitForTimeout(100),
      `closeEvidencePopup wait after outside click ${popupId}`,
    );
  }
}

async function injectEvidencePopupSnapshots(
  page: Page,
  popupSnapshots: readonly EvidencePopupSnapshot[],
): Promise<void> {
  if (popupSnapshots.length === 0) {
    return;
  }

  await ignoreOptionalActionFailure(
    page.evaluate((entries) => {
      for (const entry of entries) {
        if (document.getElementById(entry.id)) {
          continue;
        }

        const template = document.createElement("template");
        template.innerHTML = entry.html.trim();
        const element = template.content.firstElementChild;
        if (!(element instanceof HTMLElement)) {
          continue;
        }

        element.id = entry.id;
        element.hidden = false;
        element.setAttribute("aria-hidden", "false");
        element.dataset.touchBrowserEvidenceSelector = entry.label;
        element.style.display = "block";
        element.style.visibility = "visible";
        element.style.opacity = "1";
        element.style.position = "static";
        document.body.appendChild(element);
      }
    }, popupSnapshots),
    "injectEvidencePopupSnapshots append snapshots",
  );
}

function popupLocator(page: Page, popupId: string): Locator {
  return page.locator(`[id=${JSON.stringify(popupId)}]`);
}

async function selectorDescriptor(locator: Locator): Promise<string> {
  const [text, ariaLabel, name] = await Promise.all([
    readProbeFallback(locator.textContent(), "", "selectorDescriptor text"),
    readProbeFallback(
      locator.getAttribute("aria-label"),
      null,
      "selectorDescriptor aria-label",
    ),
    readProbeFallback(
      locator.getAttribute("name"),
      null,
      "selectorDescriptor name",
    ),
  ]);

  return normalizeWhitespace([text, ariaLabel, name].filter(Boolean).join(" "));
}

function looksLikeEvidenceSelector(text: string): boolean {
  const normalized = text.toLowerCase();
  return EVIDENCE_SELECTOR_KEYWORDS.some((keyword) =>
    normalized.includes(keyword),
  );
}
