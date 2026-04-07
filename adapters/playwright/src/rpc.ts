import type {
  JsonRpcFailure,
  JsonRpcId,
  JsonRpcSuccess,
  SubmitPrefillDescriptor,
} from "./types.js";

export function success(id: JsonRpcId, result: unknown): JsonRpcSuccess {
  return {
    jsonrpc: "2.0",
    id,
    result,
  };
}

export function failure(
  id: JsonRpcId,
  code: number,
  message: string,
): JsonRpcFailure {
  return {
    jsonrpc: "2.0",
    id,
    error: { code, message },
  };
}

export function asString(value: unknown): string | undefined {
  return typeof value === "string" && value.length > 0 ? value : undefined;
}

export function asNumber(value: unknown): number | undefined {
  return typeof value === "number" ? value : undefined;
}

export function asBoolean(value: unknown): boolean | undefined {
  return typeof value === "boolean" ? value : undefined;
}

export function asPositiveInteger(value: unknown): number | undefined {
  return typeof value === "number" && Number.isInteger(value) && value > 0
    ? value
    : undefined;
}

export function asSubmitPrefillDescriptors(
  value: unknown,
): SubmitPrefillDescriptor[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return value
    .map((entry) => {
      if (entry === null || typeof entry !== "object") {
        return undefined;
      }

      const record = entry as Record<string, unknown>;
      const targetRef = asString(record.targetRef);
      const textValue = asString(record.value);
      if (!targetRef || !textValue) {
        return undefined;
      }

      return {
        targetRef,
        targetText: asString(record.targetText),
        targetTagName: asString(record.targetTagName),
        targetDomPathHint: asString(record.targetDomPathHint),
        targetOrdinalHint: asPositiveInteger(record.targetOrdinalHint),
        targetName: asString(record.targetName),
        targetInputType: asString(record.targetInputType),
        value: textValue,
      } satisfies SubmitPrefillDescriptor;
    })
    .filter((entry): entry is SubmitPrefillDescriptor => entry !== undefined);
}

export async function readStdin(): Promise<string> {
  const chunks: Buffer[] = [];

  for await (const chunk of process.stdin) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }

  return Buffer.concat(chunks).toString("utf8").trim();
}
