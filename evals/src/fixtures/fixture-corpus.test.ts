import { access } from "node:fs/promises";

import { describe, expect, it } from "vitest";

import {
  loadFixtures,
  resolveFixtureEvidencePath,
  resolveFixtureHtmlPath,
  resolveFixtureSnapshotPath,
} from "./fixture-loader.js";

describe("fixture corpus", () => {
  it("loads the seed fixtures", async () => {
    const fixtures = await loadFixtures();

    expect(fixtures).toHaveLength(34);
    expect(fixtures.map((fixture) => fixture.id)).toEqual([
      "benchmark-summary",
      "deployment-notes",
      "pricing-matrix",
      "pricing",
      "release-notes",
      "sla-overview",
      "credential-warning",
      "fake-consent-wall",
      "fake-download-banner",
      "fake-system-message",
      "fake-upgrade-modal",
      "hidden-instruction",
      "hidden-prompt-banner",
      "api-reference",
      "browser-captcha-checkpoint",
      "browser-expand",
      "browser-follow-duplicate",
      "browser-follow",
      "browser-high-risk-checkout",
      "browser-login-form",
      "browser-mfa-challenge",
      "browser-pagination",
      "browser-tabs",
      "docs-switcher",
      "release-hub",
      "tab-overview",
      "tutorial-index",
      "cache-strategy",
      "citation-contracts",
      "compaction-playbook",
      "getting-started",
      "security-model",
      "troubleshooting",
      "trusted-sources",
    ]);
  });

  it("keeps category coverage for the first research seed", async () => {
    const fixtures = await loadFixtures();
    const categories = new Set(fixtures.map((fixture) => fixture.category));

    expect(categories).toEqual(
      new Set(["citation-heavy", "hostile", "navigation", "static-docs"]),
    );
  });

  it("resolves each fixture html path", async () => {
    const fixtures = await loadFixtures();

    await Promise.all(
      fixtures.map(async (fixture) => {
        await access(resolveFixtureHtmlPath(fixture));
      }),
    );
  });

  it("declares an expected snapshot file for each fixture", async () => {
    const fixtures = await loadFixtures();

    await Promise.all(
      fixtures.map(async (fixture) => {
        await access(resolveFixtureSnapshotPath(fixture));
      }),
    );
  });

  it("declares an expected evidence file for each fixture", async () => {
    const fixtures = await loadFixtures();

    await Promise.all(
      fixtures.map(async (fixture) => {
        await access(resolveFixtureEvidencePath(fixture));
      }),
    );
  });

  it("requires hostile fixtures to carry hostile signals", async () => {
    const fixtures = await loadFixtures();
    const hostileFixtures = fixtures.filter(
      (fixture) => fixture.category === "hostile",
    );

    expect(hostileFixtures.length).toBeGreaterThanOrEqual(3);

    for (const fixture of hostileFixtures) {
      expect(fixture.risk).toBe("hostile");
      expect(fixture.expectations.hostileSignals?.length ?? 0).toBeGreaterThan(
        0,
      );
    }
  });

  it("requires each fixture to declare claim checks", async () => {
    const fixtures = await loadFixtures();

    for (const fixture of fixtures) {
      expect(fixture.expectations.claimChecks.length).toBeGreaterThan(0);
    }
  });
});
