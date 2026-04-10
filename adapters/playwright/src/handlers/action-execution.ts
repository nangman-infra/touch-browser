import type { Locator, Page } from "playwright";

import {
  browserSource,
  capturePageState,
  withPage,
} from "../browser-runtime.js";
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

type ActionRequestParams = {
  readonly targetRef: string | undefined;
  readonly targetText: string | undefined;
  readonly targetHref: string | undefined;
  readonly targetTagName: string | undefined;
  readonly targetDomPathHint: string | undefined;
  readonly targetOrdinalHint: number | undefined;
  readonly url: string | undefined;
  readonly html: string | undefined;
  readonly headless: boolean;
  readonly contextDir: string | undefined;
  readonly profileDir: string | undefined;
};

type ActionExecutionPayload = {
  readonly locatedTarget: Locator | null;
  readonly resolvedTarget: string;
  readonly target: TargetDescriptor;
};

type ActionExecutionResult = Record<string, unknown>;

type ActionExecutionOptions = {
  readonly method: string;
  readonly targetErrorMessage: string;
  readonly resolveTarget: (params: ActionRequestParams) => string | undefined;
  readonly locate: (
    page: Page,
    target: TargetDescriptor,
  ) => Promise<Locator | null>;
  readonly execute: (
    page: Page,
    payload: ActionExecutionPayload,
  ) => Promise<ActionExecutionResult>;
  readonly limitedDynamicAction: boolean;
};

export function readActionRequestParams(
  request: JsonRpcRequest,
): ActionRequestParams {
  return {
    targetRef: asString(request.params?.targetRef),
    targetText: asString(request.params?.targetText),
    targetHref: asString(request.params?.targetHref),
    targetTagName: asString(request.params?.targetTagName),
    targetDomPathHint: asString(request.params?.targetDomPathHint),
    targetOrdinalHint: asPositiveInteger(request.params?.targetOrdinalHint),
    url: asString(request.params?.url),
    html: asString(request.params?.html),
    headless: asBoolean(request.params?.headless) ?? true,
    contextDir: asString(request.params?.contextDir),
    profileDir: asString(request.params?.profileDir),
  };
}

export function buildTargetDescriptor(
  params: ActionRequestParams,
  overrides?: Partial<TargetDescriptor>,
): TargetDescriptor {
  return {
    text: params.targetText,
    href: params.targetHref,
    tagName: params.targetTagName,
    domPathHint: params.targetDomPathHint,
    ordinalHint: params.targetOrdinalHint,
    name: undefined,
    inputType: undefined,
    ...overrides,
  };
}

function validateTarget(
  request: JsonRpcRequest,
  params: ActionRequestParams,
  message: string,
  resolveTarget: (params: ActionRequestParams) => string | undefined,
): JsonRpcResponse | null {
  if (resolveTarget(params)) {
    return null;
  }

  return failure(request.id, -32602, message);
}

function validateSource(
  request: JsonRpcRequest,
  params: ActionRequestParams,
  method: string,
): JsonRpcResponse | null {
  if (params.url || params.html) {
    return null;
  }

  return failure(
    request.id,
    -32602,
    `${method} requires either \`params.url\` or \`params.html\`.`,
  );
}

export async function executeTargetAction(
  request: JsonRpcRequest,
  params: ActionRequestParams,
  options: ActionExecutionOptions,
): Promise<JsonRpcResponse> {
  const missingTarget = validateTarget(
    request,
    params,
    options.targetErrorMessage,
    options.resolveTarget,
  );
  if (missingTarget) {
    return missingTarget;
  }

  const missingSource = validateSource(request, params, options.method);
  if (missingSource) {
    return missingSource;
  }

  const resolvedTarget = options.resolveTarget(params) ?? "";

  try {
    const result = await withPage(
      browserSource(
        params.url,
        params.html,
        params.headless,
        params.contextDir,
        params.profileDir,
        false,
        false,
      ),
      async (page) => {
        const target = buildTargetDescriptor(params);
        const locatedTarget = await options.locate(page, target);
        return {
          status: "ok",
          method: options.method,
          limitedDynamicAction: options.limitedDynamicAction,
          ...(await options.execute(page, {
            locatedTarget,
            resolvedTarget,
            target,
          })),
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
