import { describe, expect, it } from "vitest";

import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const reportPath = `${scenarioFixturesRoot}/live-session-synthesis/report.json`;

describe("session synthesis scenario", () => {
  it("keeps multi-page synthesis, replay, and compaction coherent across a live browser session", async () => {
    const report = await readJsonFile<{
      readonly synthesis: {
        readonly report: {
          readonly snapshotCount: number;
          readonly evidenceReportCount: number;
          readonly visitedUrls: readonly string[];
          readonly synthesizedNotes: readonly string[];
          readonly supportedClaims: ReadonlyArray<{ readonly claimId: string }>;
          readonly unsupportedClaims: ReadonlyArray<{
            readonly claimId: string;
          }>;
        };
      };
      readonly compact: {
        readonly approxTokens: number;
        readonly refIndex: readonly unknown[];
      };
      readonly replay: {
        readonly replayedActions: number;
      };
    }>(reportPath);

    expect(report.synthesis.report.snapshotCount).toBeGreaterThanOrEqual(3);
    expect(report.synthesis.report.evidenceReportCount).toBeGreaterThanOrEqual(
      2,
    );
    expect(report.synthesis.report.visitedUrls.length).toBeGreaterThanOrEqual(
      3,
    );
    expect(
      report.synthesis.report.synthesizedNotes.some((note) =>
        note.includes("Starter plan costs $29 per month."),
      ),
    ).toBe(true);
    expect(
      report.synthesis.report.supportedClaims.some(
        (claim) => claim.claimId === "c1",
      ),
    ).toBe(true);
    expect(
      report.synthesis.report.unsupportedClaims.length,
    ).toBeGreaterThanOrEqual(1);
    expect(report.compact.approxTokens).toBeGreaterThan(0);
    expect(report.compact.refIndex.length).toBeGreaterThan(0);
    expect(report.replay.replayedActions).toBeGreaterThanOrEqual(2);
  });
});
