import {
  nextPaginationSelectors,
  prevPaginationSelectors,
  settleAfterAction,
} from "../action-helpers.js";
import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
import { findFirstLocator } from "../locator-scoring.js";
import { asBoolean, asNumber, asString, failure, success } from "../rpc.js";
import { normalizeWhitespace } from "../shared.js";
import type { JsonRpcRequest, JsonRpcResponse } from "../types.js";

export async function handlePaginate(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const direction = asString(request.params?.direction);
  const currentPage = asNumber(request.params?.currentPage) ?? 1;
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);

  if (direction !== "next" && direction !== "prev") {
    return failure(
      request.id,
      -32602,
      "browser.paginate requires `params.direction` to be `next` or `prev`.",
    );
  }

  if (!url && !html) {
    return failure(
      request.id,
      -32602,
      "browser.paginate requires either `params.url` or `params.html`.",
    );
  }

  try {
    const result = await withPage(
      browserSource(url, html, headless, contextDir, profileDir, false),
      async (page) => {
        const locator = await findFirstLocator(
          page,
          direction === "next"
            ? nextPaginationSelectors()
            : prevPaginationSelectors(),
        );

        if (!locator) {
          throw new Error(`No ${direction} pagination target was found.`);
        }

        const clickedText = normalizeWhitespace(
          (await locator.textContent().catch(() => direction)) ?? direction,
        );
        await locator.click();
        await settleAfterAction(page);

        return {
          status: "ok",
          method: "browser.paginate",
          limitedDynamicAction: true,
          direction,
          page:
            direction === "next"
              ? currentPage + 1
              : Math.max(1, currentPage - 1),
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
