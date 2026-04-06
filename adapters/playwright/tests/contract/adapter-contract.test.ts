import { describe, expect, it } from "vitest";

import { adapterStatus, handleRequest } from "../support/adapter-helpers.js";

describe("playwright adapter contract", () => {
  it("reports the fixed transport contract and capabilities", () => {
    expect(adapterStatus()).toEqual({
      status: "ready",
      adapter: "playwright",
      transport: "stdio-json-rpc",
      dynamicFallback: "browser-backed-snapshot",
      browserBackedSnapshot: true,
      capabilities: [
        "adapter.status",
        "browser.snapshot",
        "browser.follow",
        "browser.click",
        "browser.type",
        "browser.submit",
        "browser.paginate",
        "browser.expand",
      ],
    });
  });

  it("handles adapter status requests", async () => {
    const response = await handleRequest({
      jsonrpc: "2.0",
      id: "req-1",
      method: "adapter.status",
    });

    expect(response).toMatchObject({
      jsonrpc: "2.0",
      id: "req-1",
      result: {
        status: "ready",
        adapter: "playwright",
      },
    });
  });
});
