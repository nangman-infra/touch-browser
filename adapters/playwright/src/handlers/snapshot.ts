import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
import { maybeExpandEvidenceSelectors } from "../evidence-selectors.js";
import { asBoolean, asNumber, asString, failure, success } from "../rpc.js";
import type {
  BrowserPageState,
  JsonRpcRequest,
  JsonRpcResponse,
} from "../types.js";

export async function handleSnapshot(
  request: JsonRpcRequest,
): Promise<JsonRpcResponse> {
  const url = asString(request.params?.url);
  const html = asString(request.params?.html);
  const budget = asNumber(request.params?.budget) ?? 1200;
  const headless = asBoolean(request.params?.headless) ?? true;
  const contextDir = asString(request.params?.contextDir);
  const profileDir = asString(request.params?.profileDir);
  const searchIdentity = asBoolean(request.params?.searchIdentity) ?? false;
  const manualRecovery = asBoolean(request.params?.manualRecovery) ?? false;

  if (!url && !html && !contextDir && !profileDir) {
    return failure(
      request.id,
      -32602,
      "browser.snapshot requires `params.url`, `params.html`, `params.contextDir`, or `params.profileDir`.",
    );
  }

  try {
    const pageState = await withPage<BrowserPageState>(
      browserSource(
        url,
        html,
        headless,
        contextDir,
        profileDir,
        searchIdentity,
        manualRecovery,
      ),
      async (page) => {
        await maybeExpandEvidenceSelectors(page);
        return capturePageState(page);
      },
    );
    return success(request.id, {
      status: "ok",
      mode: "playwright-browser",
      requestedBudget: budget,
      source: url ?? (html ? "inline-html" : "persistent-context"),
      ...pageState,
      limitedDynamicActions: [
        "browser.follow",
        "browser.paginate",
        "browser.expand",
      ],
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return failure(request.id, -32000, message);
  }
}
