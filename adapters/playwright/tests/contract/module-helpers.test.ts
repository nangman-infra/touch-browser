import { describe, expect, it } from "vitest";

import { browserSource } from "../../src/browser-runtime.js";
import {
  asBoolean,
  asNumber,
  asPositiveInteger,
  asString,
  asSubmitPrefillDescriptors,
  failure,
  success,
} from "../../src/rpc.js";
import {
  describeUnknownValue,
  extractBrowserVersion,
  normalizeWhitespace,
} from "../../src/shared.js";

describe("playwright adapter module helpers", () => {
  it("serializes JSON-RPC envelopes and primitive parameter coercion", () => {
    expect(success("req-1", { ok: true })).toEqual({
      jsonrpc: "2.0",
      id: "req-1",
      result: { ok: true },
    });
    expect(failure(null, -32601, "Unsupported method")).toEqual({
      jsonrpc: "2.0",
      id: null,
      error: {
        code: -32601,
        message: "Unsupported method",
      },
    });

    expect(asString("value")).toBe("value");
    expect(asString("")).toBeUndefined();
    expect(asString(12)).toBeUndefined();

    expect(asNumber(42)).toBe(42);
    expect(asNumber("42")).toBeUndefined();

    expect(asBoolean(true)).toBe(true);
    expect(asBoolean("true")).toBeUndefined();

    expect(asPositiveInteger(7)).toBe(7);
    expect(asPositiveInteger(0)).toBeUndefined();
    expect(asPositiveInteger(-1)).toBeUndefined();
    expect(asPositiveInteger(1.5)).toBeUndefined();
  });

  it("maps submit prefill descriptors and drops invalid records", () => {
    expect(
      asSubmitPrefillDescriptors([
        null,
        "invalid",
        {
          targetRef: "input-email",
          targetText: "Email address",
          targetTagName: "input",
          targetDomPathHint: "html > body > form",
          targetOrdinalHint: 2,
          targetName: "email",
          targetInputType: "email",
          value: "person@example.com",
        },
        {
          targetRef: "",
          value: "missing ref",
        },
        {
          targetRef: "input-password",
          value: "",
        },
      ]),
    ).toEqual([
      {
        targetRef: "input-email",
        targetText: "Email address",
        targetTagName: "input",
        targetDomPathHint: "html > body > form",
        targetOrdinalHint: 2,
        targetName: "email",
        targetInputType: "email",
        value: "person@example.com",
      },
    ]);
  });

  it("normalizes shared values and handles unknown error payloads", () => {
    expect(normalizeWhitespace("  Hello \n   world\t ")).toBe("Hello world");

    expect(describeUnknownValue("plain message", "fallback")).toBe(
      "plain message",
    );
    expect(describeUnknownValue(123, "fallback")).toBe("123");
    expect(describeUnknownValue(false, "fallback")).toBe("false");
    expect(
      describeUnknownValue(new Error("broken browser state"), "fallback"),
    ).toBe("broken browser state");
    expect(describeUnknownValue({ status: "ok", code: 200 }, "fallback")).toBe(
      '{"status":"ok","code":200}',
    );

    const circular: { self?: unknown } = {};
    circular.self = circular;
    expect(describeUnknownValue(circular, "fallback")).toBe("fallback");
    expect(describeUnknownValue(undefined, "fallback")).toBe("fallback");
  });

  it("extracts browser versions and builds browser sources verbatim", () => {
    expect(
      extractBrowserVersion(
        "Google Chrome 146.0.7423.21 Official Build arm64 on macOS",
      ),
    ).toBe("146.0.7423.21");
    expect(
      extractBrowserVersion("Chromium without a four-part version token"),
    ).toBeUndefined();

    expect(
      browserSource(
        "https://example.com/docs",
        undefined,
        true,
        "/tmp/context",
        "/tmp/profile",
        true,
        false,
      ),
    ).toEqual({
      url: "https://example.com/docs",
      html: undefined,
      contextDir: "/tmp/context",
      profileDir: "/tmp/profile",
      headless: true,
      searchIdentity: true,
      manualRecovery: false,
    });
  });
});
