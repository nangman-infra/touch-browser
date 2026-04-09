import { findClickLocator } from "../locator-scoring.js";
import type { JsonRpcRequest, JsonRpcResponse } from "../types.js";

import { executePointerAction } from "./pointer-action.js";

export async function handleClick(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  return executePointerAction(request, {
    method: "browser.click",
    targetErrorMessage:
      "browser.click requires `params.targetRef`, `params.targetText`, or `params.targetHref`.",
    missingTargetPrefix: "click",
    resultKey: "clickedRef",
    limitedDynamicAction: false,
    locate: async (page, target) =>
      (await findClickLocator(page, target)) ?? null,
  });
}
