import {
  fillTargetLocator,
  settleAfterAction,
  submitTargetLocator,
} from "../action-helpers.js";
import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
import { findSubmitLocator, findTypeLocator } from "../locator-scoring.js";
import {
  asBoolean,
  asPositiveInteger,
  asString,
  asSubmitPrefillDescriptors,
  failure,
  success,
} from "../rpc.js";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  TargetDescriptor,
} from "../types.js";

export async function handleSubmit(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);
  const prefill = asSubmitPrefillDescriptors(request.params?.prefill);

  if (!targetRef && !targetText) {
    return failure(
      request.id,
      -32602,
      "browser.submit requires `params.targetRef` or `params.targetText`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.submit requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        for (const action of prefill) {
          const fillTarget = {
            text: action.targetText,
            href: undefined,
            tagName: action.targetTagName,
            domPathHint: action.targetDomPathHint,
            ordinalHint: action.targetOrdinalHint,
            name: action.targetName,
            inputType: action.targetInputType,
          } satisfies TargetDescriptor;
          const fillLocator = await findTypeLocator(page, fillTarget);
          if (!fillLocator) {
            continue;
          }

          await fillTargetLocator(page, fillLocator, action.value);
        }

        const target = {
          text: targetText,
          href: undefined,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        } satisfies TargetDescriptor;
        const locator = await findSubmitLocator(page, target);
        if (!locator) {
          throw new Error(
            `No submit target was found for \`${resolvedTarget}\`.`,
          );
        }

        await submitTargetLocator(locator);
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.submit",
          limitedDynamicAction: false,
          submittedRef: targetRef ?? targetText,
          targetText: resolvedTarget,
          ...(await capturePageState(page)),
        };
      },
    );

    return success(request.id, result);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}
