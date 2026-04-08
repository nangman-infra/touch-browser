import type { Locator, Page } from "playwright";

import { readProbeFallback } from "./error-tolerance.js";
import { normalizeWhitespace } from "./shared.js";
import type {
  CandidateDescriptor,
  ScoredCandidate,
  TargetDescriptor,
} from "./types.js";

export async function findFirstLocator(
  page: Page,
  selectors: string[],
): Promise<ReturnType<Page["locator"]> | undefined> {
  for (const selector of selectors) {
    const locator = page.locator(selector).first();
    const count = await readProbeFallback(
      locator.count(),
      0,
      `findFirstLocator count ${selector}`,
    );
    if (count > 0) {
      return locator;
    }
  }

  return undefined;
}

export async function findExpandLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator("button, [role='button'], summary, a"),
    target,
  );
}

export async function findFollowLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(page.locator("a"), target);
}

export async function findClickLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator(
      "button, [role='button'], a, input[type='submit'], input[type='button'], input[type='checkbox'], input[type='radio']",
    ),
    target,
  );
}

export async function findTypeLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator("input, textarea, [contenteditable='true']"),
    target,
  );
}

export async function findSubmitLocator(
  page: Page,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  return findBestLocator(
    page.locator(
      "form, button[type='submit'], input[type='submit'], button, input[type='button']",
    ),
    target,
  );
}

async function findBestLocator(
  root: Locator,
  target: TargetDescriptor,
): Promise<Locator | undefined> {
  const count = await root.count();
  const candidates: ScoredCandidate[] = [];

  for (let index = 0; index < count; index += 1) {
    const locator = root.nth(index);
    const descriptor = await describeCandidate(locator, index);
    if (!descriptor) {
      continue;
    }

    const score = scoreCandidate(descriptor, target);
    if (score > 0) {
      candidates.push({ descriptor, score });
    }
  }

  candidates.sort((left, right) => {
    const scoreDiff = right.score - left.score;
    if (scoreDiff !== 0) {
      return scoreDiff;
    }

    return left.descriptor.domIndex - right.descriptor.domIndex;
  });

  if (candidates.length === 0) {
    return undefined;
  }

  const firstCandidate = candidates[0];
  if (!firstCandidate) {
    return undefined;
  }

  if (target.ordinalHint && target.ordinalHint > 1) {
    const topScore = firstCandidate.score;
    const topCandidates = candidates.filter(
      (candidate) => candidate.score === topScore,
    );
    const ordinalIndex = target.ordinalHint - 1;
    return (
      topCandidates[ordinalIndex]?.descriptor.locator ??
      candidates[ordinalIndex]?.descriptor.locator ??
      firstCandidate.descriptor.locator
    );
  }

  return firstCandidate.descriptor.locator;
}

async function describeCandidate(
  locator: Locator,
  domIndex: number,
): Promise<CandidateDescriptor | undefined> {
  const isVisible = await readProbeFallback(
    locator.isVisible(),
    false,
    `describeCandidate visible ${domIndex}`,
  );
  if (!isVisible) {
    return undefined;
  }

  const [
    text,
    href,
    tagName,
    fullPath,
    parentPath,
    name,
    inputType,
    placeholder,
    value,
    ariaLabel,
  ] = await Promise.all([
    readProbeFallback(
      locator.textContent(),
      "",
      `describeCandidate text ${domIndex}`,
    ),
    readProbeFallback(
      locator.getAttribute("href"),
      null,
      `describeCandidate href ${domIndex}`,
    ),
    readProbeFallback(
      locator.evaluate((element) => element.tagName.toLowerCase()),
      "",
      `describeCandidate tagName ${domIndex}`,
    ),
    readProbeFallback(
      locator.evaluate((element) => {
        const parts: string[] = [];
        let current: Element | null = element;
        while (current) {
          parts.unshift(current.tagName.toLowerCase());
          current = current.parentElement;
        }
        return parts.join(" > ");
      }),
      "",
      `describeCandidate fullPath ${domIndex}`,
    ),
    readProbeFallback(
      locator.evaluate((element) => {
        const parts: string[] = [];
        let current: Element | null = element.parentElement;
        while (current) {
          parts.unshift(current.tagName.toLowerCase());
          current = current.parentElement;
        }
        return parts.join(" > ");
      }),
      "",
      `describeCandidate parentPath ${domIndex}`,
    ),
    readProbeFallback(
      locator.getAttribute("name"),
      null,
      `describeCandidate name ${domIndex}`,
    ),
    readProbeFallback(
      locator.getAttribute("type"),
      null,
      `describeCandidate type ${domIndex}`,
    ),
    readProbeFallback(
      locator.getAttribute("placeholder"),
      null,
      `describeCandidate placeholder ${domIndex}`,
    ),
    readProbeFallback(
      locator.inputValue(),
      "",
      `describeCandidate value ${domIndex}`,
    ),
    readProbeFallback(
      locator.getAttribute("aria-label"),
      null,
      `describeCandidate aria-label ${domIndex}`,
    ),
  ]);

  const normalizedText = normalizeWhitespace(text ?? "");
  const inputDescriptor = normalizeWhitespace(
    [name, inputType, placeholder, value, ariaLabel]
      .map((part) => normalizeWhitespace(part ?? ""))
      .filter(Boolean)
      .join(" "),
  );
  const resolvedText = normalizedText || inputDescriptor;
  if (!resolvedText && !href) {
    return undefined;
  }

  return {
    locator,
    domIndex,
    text: resolvedText,
    href: href ?? undefined,
    tagName,
    fullPath,
    parentPath,
  };
}

function scoreCandidate(
  candidate: CandidateDescriptor,
  target: TargetDescriptor,
): number {
  const candidateText = candidate.text.toLowerCase();
  const targetText = normalizeWhitespace(target.text ?? "").toLowerCase();
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
    if (partialScore === undefined) {
      return 0;
    }
    score += partialScore;
  }

  return score;
}

function scoreTextMatch(
  candidateText: string,
  targetText: string,
): number | undefined {
  if (!targetText) {
    return 0;
  }

  if (candidateText === targetText) {
    return 5;
  }

  if (
    candidateText.includes(targetText) ||
    targetText.includes(candidateText)
  ) {
    return 3;
  }

  return undefined;
}

function scoreHrefMatch(
  candidateHref: string | undefined,
  targetHref: string | undefined,
): number | undefined {
  if (!targetHref) {
    return 0;
  }

  if (candidateHref === targetHref) {
    return 4;
  }

  return candidateHref ? undefined : 0;
}

function scoreTagNameMatch(
  candidateTagName: string,
  targetTagName: string | undefined,
): number | undefined {
  if (!targetTagName) {
    return 0;
  }

  return candidateTagName === targetTagName.toLowerCase() ? 2 : undefined;
}

function scoreContainsMatch(
  candidateText: string,
  targetValue: string | undefined,
  score: number,
): number | undefined {
  if (!targetValue) {
    return 0;
  }

  return candidateText.includes(targetValue.toLowerCase()) ? score : undefined;
}

function scoreDomPathMatch(
  candidate: CandidateDescriptor,
  domPathHint: string | undefined,
): number {
  if (!domPathHint) {
    return 0;
  }

  const normalizedHint = domPathHint.toLowerCase();
  if (candidate.parentPath === normalizedHint) {
    return 6;
  }

  if (candidate.fullPath === normalizedHint) {
    return 5;
  }

  return candidate.fullPath.startsWith(`${normalizedHint} >`) ? 2 : 0;
}
