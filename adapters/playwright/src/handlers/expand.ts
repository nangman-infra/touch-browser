import { settleAfterAction } from "../action-helpers.js";
import { findExpandLocator } from "../locator-scoring.js";
import { normalizeWhitespace } from "../shared.js";
import type { JsonRpcRequest, JsonRpcResponse } from "../types.js";

import {
  buildTargetDescriptor,
  executeTargetAction,
  readActionRequestParams,
} from "./action-execution.js";

export async function handleExpand(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const params = readActionRequestParams(request);

  return executeTargetAction(request, params, {
    method: "browser.expand",
    targetErrorMessage:
      "browser.expand requires `params.targetRef` or `params.targetText`.",
    resolveTarget: (input) => input.targetText ?? input.targetRef,
    limitedDynamicAction: true,
    locate: async (page) =>
      (await findExpandLocator(
        page,
        buildTargetDescriptor(params, {
          text: params.targetText ?? params.targetRef,
          href: undefined,
        }),
      )) ?? null,
    execute: async (page, payload) => {
      if (!payload.locatedTarget) {
        throw new Error(
          `No expandable target was found for \`${payload.resolvedTarget}\`.`,
        );
      }

      const clickedText = normalizeWhitespace(
        (await payload.locatedTarget
          .textContent()
          .catch(() => payload.resolvedTarget)) ?? payload.resolvedTarget,
      );
      await payload.locatedTarget.click();
      await settleAfterAction(page);

      return {
        expandedRef: params.targetRef ?? params.targetText,
        targetText: payload.resolvedTarget,
        clickedText,
      };
    },
  });
}
