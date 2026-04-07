export function normalizeWhitespace(value: string): string {
  return value.trim().replaceAll(/\s+/g, " ");
}

export function describeUnknownValue(value: unknown, fallback: string): string {
  if (value === null || value === undefined) {
    return fallback;
  }

  if (typeof value === "string") {
    return value;
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return String(value);
  }

  if (value instanceof Error) {
    return value.message;
  }

  try {
    const serialized = JSON.stringify(value);
    return serialized ?? fallback;
  } catch {
    return fallback;
  }
}

export function extractBrowserVersion(output: string): string | undefined {
  let token = "";

  for (const char of output) {
    if (isAsciiDigit(char) || char === ".") {
      token += char;
      continue;
    }

    const version = normalizeVersionToken(token);
    if (version) {
      return version;
    }
    token = "";
  }

  return normalizeVersionToken(token);
}

function normalizeVersionToken(token: string): string | undefined {
  if (!token.includes(".")) {
    return undefined;
  }

  const parts = token.split(".");
  if (parts.length !== 4) {
    return undefined;
  }

  return parts.every(isDigitString) ? token : undefined;
}

function isDigitString(value: string): boolean {
  return value.length > 0 && Array.from(value).every(isAsciiDigit);
}

function isAsciiDigit(char: string): boolean {
  return char >= "0" && char <= "9";
}
