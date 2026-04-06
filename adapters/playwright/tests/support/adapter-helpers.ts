import type {
  JsonRpcFailure,
  JsonRpcResponse,
  JsonRpcSuccess,
} from "../../src/index.js";

export {
  adapterStatus,
  applySearchIdentityToGlobal,
  handleRequest,
  hasSearchIdentityMarkerForTests,
  resetSearchIdentityCachesForTests,
  resolveSearchBrowserVersionForTests,
  resolveSearchLocaleForTests,
  resolveSearchUserAgentForTests,
  writeSearchIdentityMarkerForTests,
} from "../../src/index.js";

function describeFailure(response: JsonRpcFailure): string {
  return `${response.error.code}: ${response.error.message}`;
}

export function expectJsonRpcSuccess(
  response: JsonRpcResponse,
): JsonRpcSuccess {
  if (!("result" in response)) {
    throw new Error(
      `expected JSON-RPC success response, received ${describeFailure(response)}`,
    );
  }

  return response;
}

export function readVisibleText(response: JsonRpcResponse): string {
  const success = expectJsonRpcSuccess(response);
  return String(
    (success.result as { readonly visibleText?: unknown }).visibleText ?? "",
  );
}
