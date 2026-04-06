import { describe, expect, it } from "vitest";

import {
  loadContractSchemas,
  requireValidator,
} from "../contracts/schema-loader.js";
import { readJsonFile } from "../support/json.js";
import { loadFixtures, resolveFixtureSnapshotPath } from "./fixture-loader.js";

describe("golden snapshots", () => {
  it("validates each expected snapshot against the snapshot document schema", async () => {
    const registry = await loadContractSchemas();
    const validateSnapshot = requireValidator(
      registry,
      "snapshot-document.schema.json",
    );
    const fixtures = await loadFixtures();

    for (const fixture of fixtures) {
      const snapshot = await readJsonFile<object>(
        resolveFixtureSnapshotPath(fixture),
      );

      expect(validateSnapshot(snapshot), fixture.id).toBe(true);
    }
  });

  it("keeps fixture expectations aligned with the golden snapshot baseline", async () => {
    const fixtures = await loadFixtures();

    for (const fixture of fixtures) {
      const snapshot = await readJsonFile<{
        readonly source: { readonly sourceUrl: string };
        readonly blocks: ReadonlyArray<{
          readonly kind: string;
          readonly text: string;
        }>;
      }>(resolveFixtureSnapshotPath(fixture));

      expect(snapshot.source.sourceUrl).toBe(fixture.sourceUri);

      for (const expectedText of fixture.expectations.mustContainTexts) {
        expect(
          snapshot.blocks.some((block) => block.text.includes(expectedText)),
          `${fixture.id}: ${expectedText}`,
        ).toBe(true);
      }

      if (fixture.expectations.expectedKinds) {
        const kinds = new Set(snapshot.blocks.map((block) => block.kind));
        for (const expectedKind of fixture.expectations.expectedKinds) {
          expect(
            kinds.has(expectedKind),
            `${fixture.id}: ${expectedKind}`,
          ).toBe(true);
        }
      }
    }
  });
});
