import {
  handleClick,
  handleExpand,
  handleFollow,
  handlePaginate,
  handleSnapshot,
  handleSubmit,
  handleType,
} from "./handlers/index.js";
import { failure, readStdin, success } from "./rpc.js";
import type {
  AdapterStatus,
  JsonRpcRequest,
  JsonRpcResponse,
} from "./types.js";

export type {
  AdapterStatus,
  JsonRpcFailure,
  JsonRpcId,
  JsonRpcRequest,
  JsonRpcResponse,
  JsonRpcSuccess,
} from "./types.js";

export {
  applySearchIdentityToGlobal,
  hasSearchIdentityMarkerForTests,
  resetSearchIdentityCachesForTests,
  resolveSearchBrowserVersionForTests,
  resolveSearchLocaleForTests,
  resolveSearchUserAgentForTests,
  searchIdentityPlatformProfileForTests,
  writeSearchIdentityMarkerForTests,
} from "./search-identity.js";

export function adapterStatus(): AdapterStatus {
  return {
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
  };
}

export async function handleRequest(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  switch (request.method) {
    case "adapter.status":
      return success(request.id, adapterStatus());
    case "browser.snapshot":
      return handleSnapshot(request);
    case "browser.follow":
      return handleFollow(request);
    case "browser.click":
      return handleClick(request);
    case "browser.type":
      return handleType(request);
    case "browser.submit":
      return handleSubmit(request);
    case "browser.paginate":
      return handlePaginate(request);
    case "browser.expand":
      return handleExpand(request);
    default:
      return failure(
        request.id,
        -32601,
        `Unsupported method: ${request.method}`,
      );
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const explicitRequest = process.argv[2];
  const input = explicitRequest ?? (await readStdin());

  if (input) {
    const request = JSON.parse(input) as JsonRpcRequest;
    console.log(JSON.stringify(await handleRequest(request), null, 2));
  } else {
    console.log(JSON.stringify(adapterStatus(), null, 2));
  }
}
