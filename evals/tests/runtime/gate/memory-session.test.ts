import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const scenarioRoot = `${scenarioFixturesRoot}/memory-20-step`;

describe("memory 20-step baseline", () => {
  it("keeps working memory bounded across the generated session", async () => {
    const summary = await readJsonFile<{
      readonly requestedActions: number;
      readonly actionCount: number;
      readonly sessionState: {
        readonly snapshotIds: readonly string[];
      };
      readonly memorySummary: {
        readonly turnCount: number;
        readonly maxWorkingSetSize: number;
        readonly finalWorkingSetSize: number;
        readonly visitedUrls: readonly string[];
        readonly synthesizedNotes: readonly string[];
      };
    }>(`${scenarioRoot}/summary.json`);

    expect(summary.requestedActions).toBe(20);
    expect(summary.actionCount).toBe(20);
    expect(summary.memorySummary.turnCount).toBe(20);
    expect(summary.memorySummary.maxWorkingSetSize).toBeLessThanOrEqual(6);
    expect(summary.memorySummary.finalWorkingSetSize).toBeLessThanOrEqual(6);
    expect(summary.memorySummary.visitedUrls.length).toBe(3);
    expect(summary.sessionState.snapshotIds.length).toBe(10);
    expect(summary.memorySummary.synthesizedNotes).toContain(
      "The Starter plan costs $29 per month.",
    );
    expect(summary.memorySummary.synthesizedNotes).toContain(
      "Snapshot responses include stable refs and evidence metadata.",
    );
  });
});
