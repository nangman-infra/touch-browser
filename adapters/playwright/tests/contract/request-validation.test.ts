import { describe, expect, it } from "vitest";

import { handleRequest } from "../support/adapter-helpers.js";

describe("playwright adapter request validation", () => {
  it("rejects malformed snapshot requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-5",
      method: "browser.snapshot",
      params: {},
    });

    expect(response).toEqual({
      jsonrpc: "2.0",
      id: "req-5",
      error: {
        code: -32602,
        message:
          "browser.snapshot requires `params.url`, `params.html`, `params.contextDir`, or `params.profileDir`.",
      },
    });
  });

  it("rejects malformed paginate requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-6",
      method: "browser.paginate",
      params: {
        direction: "next",
      },
    });

    expect(response).toEqual({
      jsonrpc: "2.0",
      id: "req-6",
      error: {
        code: -32602,
        message:
          "browser.paginate requires either `params.url` or `params.html`.",
      },
    });
  });

  it("rejects malformed follow requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-7",
      method: "browser.follow",
      params: {
        targetRef: "rmain:link:advanced-guide",
      },
    });

    expect(response).toEqual({
      jsonrpc: "2.0",
      id: "req-7",
      error: {
        code: -32602,
        message:
          "browser.follow requires either `params.url` or `params.html`.",
      },
    });
  });
});
