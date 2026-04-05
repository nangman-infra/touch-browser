import { describe, expect, it } from "vitest";

import {
  loadContractSchemas,
  requireValidator,
} from "../contracts/schema-loader.js";
import { readJsonFile } from "../support/json.js";
import { loadFixtures, resolveFixtureEvidencePath } from "./fixture-loader.js";

describe("golden evidence", () => {
  it("validates each expected evidence report against the evidence report schema", async () => {
    const registry = await loadContractSchemas();
    const validateEvidenceReport = requireValidator(
      registry,
      "evidence-report.schema.json",
    );
    const fixtures = await loadFixtures();

    for (const fixture of fixtures) {
      const evidence = await readJsonFile<object>(
        resolveFixtureEvidencePath(fixture),
      );
      expect(validateEvidenceReport(evidence), fixture.id).toBe(true);
    }
  });

  it("keeps supported and unsupported claims aligned with fixture expectations", async () => {
    const fixtures = await loadFixtures();

    for (const fixture of fixtures) {
      const evidence = await readJsonFile<{
        readonly source: {
          readonly sourceUrl: string;
          readonly sourceRisk: string;
        };
        readonly evidenceSupportedClaims: ReadonlyArray<{
          readonly claimId: string;
        }>;
        readonly insufficientEvidenceClaims: ReadonlyArray<{
          readonly claimId: string;
        }>;
      }>(resolveFixtureEvidencePath(fixture));

      expect(evidence.source.sourceUrl).toBe(fixture.sourceUri);
      expect(evidence.source.sourceRisk).toBe(fixture.risk);

      const supportedIds = new Set(
        evidence.evidenceSupportedClaims.map((claim) => claim.claimId),
      );
      const unsupportedIds = new Set(
        evidence.insufficientEvidenceClaims.map((claim) => claim.claimId),
      );

      for (const claimCheck of fixture.expectations.claimChecks) {
        if (claimCheck.expectedStatus === "supported") {
          expect(
            supportedIds.has(claimCheck.id),
            `${fixture.id}: ${claimCheck.id}`,
          ).toBe(true);
        } else {
          expect(
            unsupportedIds.has(claimCheck.id),
            `${fixture.id}: ${claimCheck.id}`,
          ).toBe(true);
        }
      }
    }
  });
});
