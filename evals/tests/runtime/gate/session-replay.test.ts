import { describe, expect, it } from "vitest";

import {
  loadContractSchemas,
  requireValidator,
} from "../contracts/schema-loader.js";
import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

const scenarioRoot = `${scenarioFixturesRoot}/read-only-pricing`;

describe("read-only session replay baseline", () => {
  it("validates the generated session state and replay transcript", async () => {
    const registry = await loadContractSchemas();
    const validateSessionState = requireValidator(
      registry,
      "session-state.schema.json",
    );
    const validateTranscript = requireValidator(
      registry,
      "replay-transcript.schema.json",
    );

    const sessionState = await readJsonFile<object>(
      `${scenarioRoot}/session-state.json`,
    );
    const replayTranscript = await readJsonFile<object>(
      `${scenarioRoot}/replay-transcript.json`,
    );

    expect(validateSessionState(sessionState)).toBe(true);
    expect(validateTranscript(replayTranscript)).toBe(true);
  });

  it("keeps the canonical read-only action sequence", async () => {
    const sessionState = await readJsonFile<{
      readonly currentUrl: string | null;
      readonly snapshotIds: readonly string[];
      readonly workingSetRefs: readonly string[];
    }>(`${scenarioRoot}/session-state.json`);
    const replayTranscript = await readJsonFile<{
      readonly entries: ReadonlyArray<{
        readonly kind: string;
        readonly payloadType: string;
        readonly payload: {
          readonly action?: string;
        };
      }>;
    }>(`${scenarioRoot}/replay-transcript.json`);

    const actionSequence = replayTranscript.entries
      .filter(
        (entry) =>
          entry.kind === "command" && entry.payloadType === "action-command",
      )
      .map((entry) => entry.payload.action);

    expect(actionSequence).toEqual([
      "open",
      "read",
      "follow",
      "extract",
      "diff",
      "compact",
    ]);
    expect(sessionState.currentUrl).toBe(
      "fixture://research/citation-heavy/pricing",
    );
    expect(sessionState.snapshotIds).toEqual([
      "snap_scenario001_1",
      "snap_scenario001_2",
    ]);
    expect(sessionState.workingSetRefs.length).toBe(3);
  });
});
