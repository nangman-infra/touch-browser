import type { Locator, Page } from "playwright";

import { resolveSafeFollowUrl, settleAfterAction } from "../action-helpers.js";
import { readProbeFallback } from "../error-tolerance.js";
import { normalizeWhitespace } from "../shared.js";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  TargetDescriptor,
} from "../types.js";

import {
  executeTargetAction,
  readActionRequestParams,
} from "./action-execution.js";

type PointerActionResultKey = "clickedRef" | "followedRef";

type PointerActionConfig = {
  readonly method: "browser.click" | "browser.follow";
  readonly targetErrorMessage: string;
  readonly missingTargetPrefix: "click" | "follow";
  readonly resultKey: PointerActionResultKey;
  readonly limitedDynamicAction: boolean;
  readonly locate: (
    page: Page,
    target: TargetDescriptor,
  ) => Promise<Locator | null>;
};

export async function executePointerAction(
  request: JsonRpcRequest,
  config: PointerActionConfig,
): Promise<JsonRpcResponse> {
  const params = readActionRequestParams(request);

  return executeTargetAction(request, params, {
    method: config.method,
    targetErrorMessage: config.targetErrorMessage,
    resolveTarget: (input) =>
      input.targetText ?? input.targetHref ?? input.targetRef,
    limitedDynamicAction: config.limitedDynamicAction,
    locate: async (page, target) => (await config.locate(page, target)) ?? null,
    execute: async (page, payload) => {
      let clickedText = payload.resolvedTarget;

      if (payload.locatedTarget) {
        clickedText = normalizeWhitespace(
          (await readProbeFallback(
            payload.locatedTarget.textContent(),
            payload.resolvedTarget,
            `${config.method} clickedText`,
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
            `No ${config.missingTargetPrefix} target was found for \`${payload.resolvedTarget}\`.`,
          );
        }

        await page.goto(fallbackUrl, { waitUntil: "domcontentloaded" });
        await settleAfterAction(page);
      }

      return {
        [config.resultKey]:
          params.targetRef ?? params.targetText ?? params.targetHref,
        targetText: payload.resolvedTarget,
        targetHref: params.targetHref,
        clickedText,
      };
    },
  });
}
