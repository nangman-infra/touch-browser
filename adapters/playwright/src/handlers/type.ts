import { fillTargetLocator, settleAfterAction } from "../action-helpers.js";
import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
import { findTypeLocator } from "../locator-scoring.js";
import {
  asBoolean,
  asPositiveInteger,
  asString,
  failure,
  success,
} from "../rpc.js";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  TargetDescriptor,
} from "../types.js";

export async function handleType(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetTagName = asString(request.params?.targetTagName);
  const targetDomPathHint = asString(request.params?.targetDomPathHint);
  const targetOrdinalHint = asPositiveInteger(
    request.params?.targetOrdinalHint,
  );
  const targetName = asString(request.params?.targetName);
  const targetInputType = asString(request.params?.targetInputType);
  const value = asString(request.params?.value);
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (!targetRef && !targetText) {
    return failure(
      request.id,
      -32602,
      "browser.type requires `params.targetRef` or `params.targetText`.",
    );
  }

  if (!value) {
    return failure(request.id, -32602, "browser.type requires `params.value`.");
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.type requires either `params.url` or `params.html`.",
    );
  }

  try {
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const target = {
          text: targetText,
          href: undefined,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: targetName,
          inputType: targetInputType,
        } satisfies TargetDescriptor;
        const locator = await findTypeLocator(page, target);
        if (!locator) {
          throw new Error(
            `No input target was found for \`${targetRef ?? targetText}\`.`,
          );
        }

        await fillTargetLocator(page, locator, value);
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.type",
          limitedDynamicAction: false,
          typedRef: targetRef ?? targetText,
          targetText: targetText ?? targetRef,
          typedLength: value.length,
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
