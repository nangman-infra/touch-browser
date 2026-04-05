import { countTokens } from "gpt-tokenizer/model/gpt-4o";

import {
  renderCompactSnapshot,
  renderReadingCompactSnapshot,
} from "./compact-snapshot.mjs";
import { roundTo } from "./live-sample-server.mjs";

export function estimateTokens(text) {
  return Math.max(1, Math.ceil(text.length / 4));
}

export function buildObservationTokenMetrics({
  snapshot,
  rawHtmlTokens,
  cleanedDomTokens,
  visibleTextTokens,
  mustContainTexts,
}) {
  const compactSnapshot = renderCompactSnapshot(snapshot);
  const readingCompactSnapshot = renderReadingCompactSnapshot(snapshot);
  const compactSnapshotTokenizerTokens = countTokens(compactSnapshot);
  const readingCompactSnapshotTokenizerTokens = countTokens(
    readingCompactSnapshot,
  );
  const mustContainHits = countMustContainHits(snapshot, mustContainTexts);

  return {
    compactSnapshotTokenizerTokens,
    readingCompactSnapshotTokenizerTokens,
    htmlTokenizerReductionRatio: roundTo(
      rawHtmlTokens / Math.max(compactSnapshotTokenizerTokens, 1),
      2,
    ),
    cleanedDomTokenizerReductionRatio: roundTo(
      cleanedDomTokens / Math.max(compactSnapshotTokenizerTokens, 1),
      2,
    ),
    visibleTextTokenizerReductionRatio: roundTo(
      visibleTextTokens / Math.max(compactSnapshotTokenizerTokens, 1),
      2,
    ),
    readingHtmlTokenizerReductionRatio: roundTo(
      rawHtmlTokens / Math.max(readingCompactSnapshotTokenizerTokens, 1),
      2,
    ),
    readingCleanedDomTokenizerReductionRatio: roundTo(
      cleanedDomTokens / Math.max(readingCompactSnapshotTokenizerTokens, 1),
      2,
    ),
    readingVisibleTextTokenizerReductionRatio: roundTo(
      visibleTextTokens / Math.max(readingCompactSnapshotTokenizerTokens, 1),
      2,
    ),
    mustContainRecall: roundTo(
      mustContainHits / Math.max(mustContainTexts.length, 1),
      2,
    ),
    blockCount: snapshot.blocks.length,
  };
}

export function averageMetric(items, selector) {
  if (items.length === 0) {
    return 0;
  }

  return roundTo(
    items.reduce((sum, item) => sum + pickMetric(item, selector), 0) /
      items.length,
    2,
  );
}

export function medianMetric(items, selector) {
  const values = items
    .map((item) => pickMetric(item, selector))
    .sort((left, right) => left - right);

  return roundTo(values[Math.floor(values.length / 2)] ?? 0, 2);
}

export function minMetric(items, selector) {
  const values = items
    .map((item) => pickMetric(item, selector))
    .sort((left, right) => left - right);

  return roundTo(values[0] ?? 0, 2);
}

function countMustContainHits(snapshot, mustContainTexts) {
  return mustContainTexts.filter((text) =>
    snapshot.blocks.some((block) => String(block.text).includes(text)),
  ).length;
}

function pickMetric(item, selector) {
  return typeof selector === "function" ? selector(item) : item[selector];
}
