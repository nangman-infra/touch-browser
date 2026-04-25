import { readFileSync } from "node:fs";
import path from "node:path";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";

describe("qa site regression manifest", () => {
  it("pins the official target site set and expected verdict surface", () => {
    const manifestPath = path.join(
      repoRoot,
      "fixtures",
      "scenarios",
      "qa-site-regression",
      "target-sites.json",
    );
    const manifest = JSON.parse(readFileSync(manifestPath, "utf8")) as {
      version: string;
      sites: Array<{
        id: string;
        label: string;
        target: string;
        mode: "stateless-extract" | "session-follow";
        verifierCommand?: string;
        claims: Array<{
          statement: string;
          expectedVerdict:
            | "evidence-supported"
            | "contradicted"
            | "insufficient-evidence"
            | "needs-more-browsing";
        }>;
        readView?: {
          mustContain: string[];
          mustNotContain: string[];
        };
        sessionFlow?: {
          followRef: string;
          afterReadMustContain: string[];
        };
        expected: {
          supported: boolean;
          contradicted: boolean;
          needsMoreBrowsing: boolean;
          policy: {
            pageDecision: string;
            actionDecision: string;
          };
        };
      }>;
    };

    expect(manifest.version).toBe("1.1.0");
    expect(manifest.sites.map((site) => site.id)).toEqual([
      "mdn-reference",
      "chrome-developers-blog",
      "iana-docs",
      "multi-page-follow-flow",
    ]);

    for (const site of manifest.sites) {
      expect(site.label.length).toBeGreaterThan(0);
      expect(site.target.length).toBeGreaterThan(0);
      expect(["stateless-extract", "session-follow"]).toContain(site.mode);
      expect(site.claims.length).toBeGreaterThan(0);
      for (const claim of site.claims) {
        expect(claim.statement.length).toBeGreaterThan(0);
        expect([
          "evidence-supported",
          "contradicted",
          "insufficient-evidence",
          "needs-more-browsing",
        ]).toContain(claim.expectedVerdict);
      }
      if (site.readView) {
        expect(site.readView.mustContain.length).toBeGreaterThan(0);
      }
      if (site.mode === "session-follow") {
        expect(site.sessionFlow?.followRef.length).toBeGreaterThan(0);
        expect(site.sessionFlow?.afterReadMustContain.length).toBeGreaterThan(
          0,
        );
      }
      expect(["allow", "review", "block"]).toContain(
        site.expected.policy.pageDecision,
      );
      expect(["allow", "review", "block"]).toContain(
        site.expected.policy.actionDecision,
      );
    }
  });
});
