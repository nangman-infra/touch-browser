import { settleAfterAction } from "../action-helpers.js";
import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
import { findExpandLocator } from "../locator-scoring.js";
import {
  asBoolean,
  asPositiveInteger,
  asString,
  failure,
  success,
} from "../rpc.js";
import { normalizeWhitespace } from "../shared.js";
import type {
  JsonRpcRequest,
  JsonRpcResponse,
  TargetDescriptor,
} from "../types.js";

export async function handleExpand(
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

  if (!targetRef && !targetText) {
    return failure(
      request.id,
      -32602,
      "browser.expand requires `params.targetRef` or `params.targetText`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.expand requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const locator = await findExpandLocator(page, {
          text: targetText ?? targetRef,
          href: undefined,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        } satisfies TargetDescriptor);
        if (!locator) {
          throw new Error(
            `No expandable target was found for \`${resolvedTarget}\`.`,
          );
        }

        const clickedText = normalizeWhitespace(
          (await locator.textContent().catch(() => resolvedTarget)) ??
            resolvedTarget,
        );
        await locator.click();
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.expand",
          limitedDynamicAction: true,
          expandedRef: targetRef ?? targetText,
          targetText: resolvedTarget,
          clickedText,
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
