import { resolveSafeFollowUrl, settleAfterAction } from "../action-helpers.js";
import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
import { findFollowLocator } from "../locator-scoring.js";
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

export async function handleFollow(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const targetRef = asString(request.params?.targetRef);
  const targetText = asString(request.params?.targetText);
  const targetHref = asString(request.params?.targetHref);
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

  if (!targetRef && !targetText && !targetHref) {
    return failure(
      request.id,
      -32602,
      "browser.follow requires `params.targetRef`, `params.targetText`, or `params.targetHref`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.follow requires either `params.url` or `params.html`.",
    );
  }

  try {
    const resolvedTarget = targetText ?? targetHref ?? targetRef ?? "";
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const target = {
          text: targetText,
          href: targetHref,
          tagName: targetTagName,
          domPathHint: targetDomPathHint,
          ordinalHint: targetOrdinalHint,
          name: undefined,
          inputType: undefined,
        } satisfies TargetDescriptor;
        const locator = await findFollowLocator(page, target);
        let clickedText = resolvedTarget;

        if (locator) {
          clickedText = normalizeWhitespace(
            (await locator.textContent().catch(() => resolvedTarget)) ??
              resolvedTarget,
          );
          await locator.click();
          await settleAfterAction(page);
        } else {
          const fallbackUrl = resolveSafeFollowUrl(page.url(), target.href);
          if (!fallbackUrl) {
            throw new Error(
              `No follow target was found for \`${resolvedTarget}\`.`,
            );
          }

          await page.goto(fallbackUrl, { waitUntil: "domcontentloaded" });
          await settleAfterAction(page);
        }

        return {
          status: "ok",
          method: "browser.follow",
          limitedDynamicAction: true,
          followedRef: targetRef ?? targetText ?? targetHref,
          targetText: resolvedTarget,
          targetHref,
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
