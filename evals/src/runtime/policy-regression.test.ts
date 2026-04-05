import { describe, expect, it } from "vitest";

import {
  loadContractSchemas,
  requireValidator,
} from "../contracts/schema-loader.js";
import { readJsonFile } from "../support/json.js";
import { scenarioFixturesRoot } from "../support/paths.js";

describe("policy regression scenarios", () => {
  it("keeps the static docs policy report low risk", async () => {
    const registry = await loadContractSchemas();
    const validate = requireValidator(registry, "policy-report.schema.json");
    const output = await readJsonFile<{
      readonly policy: object;
      readonly sessionState: {
        readonly currentUrl: string | null;
      };
    }>(`${scenarioFixturesRoot}/policy-static-docs/report.json`);

    expect(validate(output.policy)).toBe(true);
    expect(output.policy).toMatchObject({
      decision: "allow",
      sourceRisk: "low",
      riskClass: "low",
      blockedRefs: [],
    });
    expect(output.sessionState.currentUrl).toBe(
      "fixture://research/static-docs/getting-started",
    );
  });

  it("keeps the hostile fake system fixture blocked", async () => {
    const registry = await loadContractSchemas();
    const validate = requireValidator(registry, "policy-report.schema.json");
    const output = await readJsonFile<{
      readonly policy: {
        readonly blockedRefs: readonly string[];
      };
      readonly sessionState: {
        readonly currentUrl: string | null;
      };
    }>(`${scenarioFixturesRoot}/policy-hostile-fake-system/report.json`);

    expect(validate(output.policy)).toBe(true);
    expect(output.policy.blockedRefs).toContain(
      "rmain:link:https-malicious-example-submit",
    );
    expect(output.policy).toMatchObject({
      decision: "block",
      sourceRisk: "hostile",
      riskClass: "blocked",
    });
    expect(output.sessionState.currentUrl).toBe(
      "fixture://research/hostile/fake-system-message",
    );
  });

  it("marks the CAPTCHA checkpoint for review", async () => {
    const registry = await loadContractSchemas();
    const validate = requireValidator(registry, "policy-report.schema.json");
    const output = await readJsonFile<{
      readonly policy: {
        readonly signals: ReadonlyArray<{ readonly kind: string }>;
      };
    }>(`${scenarioFixturesRoot}/policy-navigation-captcha/report.json`);

    expect(validate(output.policy)).toBe(true);
    expect(output.policy).toMatchObject({
      decision: "review",
      sourceRisk: "low",
      riskClass: "high",
    });
    expect(
      output.policy.signals.some((signal) => signal.kind === "bot-challenge"),
    ).toBe(true);
  });

  it("marks the MFA checkpoint for review", async () => {
    const registry = await loadContractSchemas();
    const validate = requireValidator(registry, "policy-report.schema.json");
    const output = await readJsonFile<{
      readonly policy: {
        readonly signals: ReadonlyArray<{ readonly kind: string }>;
      };
    }>(`${scenarioFixturesRoot}/policy-navigation-mfa/report.json`);

    expect(validate(output.policy)).toBe(true);
    expect(output.policy).toMatchObject({
      decision: "review",
      sourceRisk: "low",
      riskClass: "high",
    });
    expect(
      output.policy.signals.some((signal) => signal.kind === "mfa-challenge"),
    ).toBe(true);
    expect(
      output.policy.signals.some(
        (signal) => signal.kind === "sensitive-auth-flow",
      ),
    ).toBe(true);
  });

  it("marks the checkout checkpoint as high-risk write", async () => {
    const registry = await loadContractSchemas();
    const validate = requireValidator(registry, "policy-report.schema.json");
    const output = await readJsonFile<{
      readonly policy: {
        readonly signals: ReadonlyArray<{ readonly kind: string }>;
      };
    }>(`${scenarioFixturesRoot}/policy-navigation-high-risk/report.json`);

    expect(validate(output.policy)).toBe(true);
    expect(output.policy).toMatchObject({
      decision: "review",
      sourceRisk: "low",
      riskClass: "high",
    });
    expect(
      output.policy.signals.some((signal) => signal.kind === "high-risk-write"),
    ).toBe(true);
  });
});
