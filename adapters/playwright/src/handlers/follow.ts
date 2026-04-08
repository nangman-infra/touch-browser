import { resolveSafeFollowUrl, settleAfterAction } from "../action-helpers.js";
import { readProbeFallback } from "../error-tolerance.js";
import { findFollowLocator } from "../locator-scoring.js";
import { normalizeWhitespace } from "../shared.js";
import type { JsonRpcRequest, JsonRpcResponse } from "../types.js";

import {
  executeTargetAction,
  readActionRequestParams,
} from "./action-execution.js";

export async function handleFollow(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const params = readActionRequestParams(request);

  return executeTargetAction(request, params, {
    method: "browser.follow",
    targetErrorMessage:
      "browser.follow requires `params.targetRef`, `params.targetText`, or `params.targetHref`.",
    resolveTarget: (input) =>
      input.targetText ?? input.targetHref ?? input.targetRef,
    limitedDynamicAction: true,
    locate: async (page, target) =>
      (await findFollowLocator(page, target)) ?? null,
    execute: async (page, payload) => {
      let clickedText = payload.resolvedTarget;

      if (payload.locatedTarget) {
        clickedText = normalizeWhitespace(
          (await readProbeFallback(
            payload.locatedTarget.textContent(),
            payload.resolvedTarget,
            "handleFollow clickedText",
          )) ?? payload.resolvedTarget,
        );
        await payload.locatedTarget.click();
        await settleAfterAction(page);
      } else {
        const fallbackUrl = resolveSafeFollowUrl(
          page.url(),
          payload.target.href,
        );
        if (!fallbackUrl) {
          throw new Error(
            `No follow target was found for \`${payload.resolvedTarget}\`.`,
          );
        }

        await page.goto(fallbackUrl, { waitUntil: "domcontentloaded" });
        await settleAfterAction(page);
      }

      return {
        followedRef: params.targetRef ?? params.targetText ?? params.targetHref,
        targetText: payload.resolvedTarget,
        targetHref: params.targetHref,
        clickedText,
      };
    },
  });
}
