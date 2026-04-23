import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { mkdtemp, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

import type { Download, Locator, Page } from "playwright";

import { resolveSafeFollowUrl, settleAfterAction } from "../action-helpers.js";
import { performClosedShadowActionAcrossFrames } from "../dom-instrumentation.js";
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

const DOWNLOAD_WAIT_MS = 2500;

type PointerActionResultKey = "clickedRef" | "followedRef";

type DownloadEvidence = {
  readonly completed: boolean;
  readonly suggestedFilename: string;
  readonly path?: string;
  readonly byteLength?: number;
  readonly sha256?: string;
  readonly failure?: string;
};

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
        const downloadPromise = page
          .waitForEvent("download", { timeout: DOWNLOAD_WAIT_MS })
          .catch(() => null);
        await payload.locatedTarget.click();
        const download = await downloadPromise;
        await settleAfterAction(page);

        const downloadEvidence = await captureDownloadEvidence(download);

        return {
          [config.resultKey]:
            params.targetRef ?? params.targetText ?? params.targetHref,
          targetText: payload.resolvedTarget,
          targetHref: params.targetHref,
          clickedText,
          ...(downloadEvidence ? { download: downloadEvidence } : {}),
        };
      }

      const shadowAction = await performClosedShadowActionAcrossFrames(page, {
        kind: config.method === "browser.follow" ? "follow" : "click",
        target: payload.target,
      });
      if (shadowAction) {
        await settleAfterAction(page);
        return {
          [config.resultKey]:
            params.targetRef ?? params.targetText ?? params.targetHref,
          targetText: shadowAction.targetText,
          targetHref: shadowAction.targetHref ?? params.targetHref,
          clickedText: shadowAction.clickedText ?? clickedText,
        };
      }

      const fallbackUrl = resolveSafeFollowUrl(page.url(), payload.target.href);
      if (!fallbackUrl) {
        throw new Error(
          `No ${config.missingTargetPrefix} target was found for \`${payload.resolvedTarget}\`.`,
        );
      }

      await page.goto(fallbackUrl, { waitUntil: "domcontentloaded" });
      await settleAfterAction(page);

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

async function captureDownloadEvidence(
  download: Download | null,
): Promise<DownloadEvidence | undefined> {
  if (!download) {
    return undefined;
  }

  const suggestedFilename = download.suggestedFilename();
  const failure = await download
    .failure()
    .catch((error: unknown) =>
      error instanceof Error ? error.message : String(error),
    );
  if (failure) {
    return {
      completed: false,
      suggestedFilename,
      failure,
    };
  }

  const downloadRoot = await mkdtemp(
    path.join(os.tmpdir(), "touch-browser-download-"),
  );
  const destination = path.join(
    downloadRoot,
    sanitizeDownloadFilename(suggestedFilename),
  );
  await download.saveAs(destination);
  const metadata = await stat(destination);

  return {
    completed: true,
    suggestedFilename,
    path: destination,
    byteLength: metadata.size,
    sha256: await sha256File(destination),
  };
}

function sanitizeDownloadFilename(filename: string): string {
  const sanitized = filename.replace(/[^\w.-]+/g, "_").replace(/^_+|_+$/g, "");
  return sanitized || "download.bin";
}

function sha256File(filePath: string): Promise<string> {
  return new Promise((resolve, reject) => {
    const hash = createHash("sha256");
    const stream = createReadStream(filePath);
    stream.on("error", reject);
    stream.on("data", (chunk) => hash.update(chunk));
    stream.on("end", () => resolve(hash.digest("hex")));
  });
}
