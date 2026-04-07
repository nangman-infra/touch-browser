import type { Locator, Page } from "playwright";

import { settleAfterAction } from "./action-helpers.js";
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
  const candidateCount = await selectorLocator.count().catch(() => 0);

  for (let index = 0; index < candidateCount; index += 1) {
    const locator = selectorLocator.nth(index);
    const descriptor = await selectorDescriptor(locator);
    candidates.push({
      index,
      descriptor,
      cacheKey: descriptor.toLowerCase(),
      popupId: await locator.getAttribute("aria-controls").catch(() => null),
    });
  }

  return candidates;
}

async function isLocatorVisible(locator: Locator): Promise<boolean> {
  return locator.isVisible().catch(() => false);
}

async function openEvidenceSelectorAndCapturePopup(
  page: Page,
  target: EvidenceSelectorTarget,
): Promise<EvidencePopupSnapshot | undefined> {
  await target.locator.click().catch(() => {});
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
  const popupHtml = await popupLocator(page, popupId)
    .evaluate((popup) => {
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
    })
    .catch(() => undefined);

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
  await page.keyboard.press("Escape").catch(() => {});
  await page.waitForTimeout(100).catch(() => {});
  const popup = popupLocator(page, popupId);
  const [popupStillVisible, hiddenAttribute, ariaHidden, dataState] =
    await Promise.all([
      popup.isVisible().catch(() => false),
      popup.getAttribute("hidden").catch(() => null),
      popup.getAttribute("aria-hidden").catch(() => null),
      popup
        .evaluate((node) =>
          node instanceof HTMLElement ? (node.dataset.state ?? null) : null,
        )
        .catch(() => null),
    ]);
  const popupStillOpen =
    popupStillVisible &&
    hiddenAttribute === null &&
    ariaHidden !== "true" &&
    dataState !== "closed";

  if (popupStillOpen) {
    await page.mouse.click(8, 8).catch(() => {});
    await page.waitForTimeout(100).catch(() => {});
  }
}

async function injectEvidencePopupSnapshots(
  page: Page,
  popupSnapshots: readonly EvidencePopupSnapshot[],
): Promise<void> {
  if (popupSnapshots.length === 0) {
    return;
  }

  await page
    .evaluate((entries) => {
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
    }, popupSnapshots)
    .catch(() => {});
}

function popupLocator(page: Page, popupId: string): Locator {
  return page.locator(`[id=${JSON.stringify(popupId)}]`);
}

async function selectorDescriptor(locator: Locator): Promise<string> {
  const [text, ariaLabel, name] = await Promise.all([
    locator.textContent().catch(() => ""),
    locator.getAttribute("aria-label").catch(() => null),
    locator.getAttribute("name").catch(() => null),
  ]);

  return normalizeWhitespace([text, ariaLabel, name].filter(Boolean).join(" "));
}

function looksLikeEvidenceSelector(text: string): boolean {
  const normalized = text.toLowerCase();
  return EVIDENCE_SELECTOR_KEYWORDS.some((keyword) =>
    normalized.includes(keyword),
  );
}
