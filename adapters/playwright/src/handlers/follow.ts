import { findFollowLocator } from "../locator-scoring.js";
import type { JsonRpcRequest, JsonRpcResponse } from "../types.js";

import { executePointerAction } from "./pointer-action.js";

export async function handleFollow(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  return executePointerAction(request, {
    method: "browser.follow",
    targetErrorMessage:
      "browser.follow requires `params.targetRef`, `params.targetText`, or `params.targetHref`.",
    missingTargetPrefix: "follow",
    resultKey: "followedRef",
    limitedDynamicAction: true,
    locate: async (page, target) =>
      (await findFollowLocator(page, target)) ?? null,
  });
}
