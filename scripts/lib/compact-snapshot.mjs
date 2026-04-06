export function renderCompactSnapshot(snapshot) {
  const hasHeading = snapshot.blocks.some((block) => block.kind === "heading");
  return snapshot.blocks
    .filter((block) => keepCompactBlock(block, hasHeading))
    .map(renderCompactBlock)
    .join("\n");
}

export function renderReadingCompactSnapshot(snapshot) {
  const hasHeading = snapshot.blocks.some((block) => block.kind === "heading");
  return snapshot.blocks
    .filter((block) => keepReadingBlock(block, hasHeading))
    .map(renderCompactBlock)
    .join("\n");
}

function renderCompactBlock(block) {
  const attrs = compactAttrFragment(block);
  const digest = compactTextDigest(String(block.text), block.kind);

  return [compactKindCode(block), attrs, digest].filter(Boolean).join(" ");
}

function keepCompactBlock(block, hasHeading) {
  switch (block.kind) {
    case "metadata":
      return !hasHeading;
    case "heading":
    case "table":
      return true;
    case "text":
      return isSalientTextBlock(String(block.text ?? ""));
    case "list":
      return (
        block.role === "content" ||
        block.role === "supporting" ||
        isSalientTextBlock(String(block.text ?? ""))
      );
    case "link":
      return (
        (block.role === "content" || block.role === "cta") &&
        isSalientTextBlock(String(block.text ?? ""))
      );
    case "button":
      return (
        block.role === "cta" && isSalientTextBlock(String(block.text ?? ""))
      );
    default:
      return false;
  }
}

function keepReadingBlock(block, hasHeading) {
  if (hasHeading && block.kind === "metadata") {
    return false;
  }

  switch (block.kind) {
    case "heading":
    case "table":
      return true;
    case "text":
      return isSalientTextBlock(String(block.text ?? ""));
    case "list":
      return (
        block.role === "content" ||
        block.role === "supporting" ||
        isSalientTextBlock(String(block.text ?? ""))
      );
    case "link":
      return (
        block.role === "content" && isSalientTextBlock(String(block.text ?? ""))
      );
    default:
      return false;
  }
}

function compactKindCode(block) {
  switch (block.kind) {
    case "metadata":
      return "m";
    case "heading":
      return `h${block.attributes?.level ?? 0}`;
    case "text":
      return "t";
    case "link":
      return "a";
    case "list":
      return "l";
    case "table":
      return "tb";
    case "form":
      return "f";
    case "button":
      return "b";
    case "input":
      return "i";
    default:
      return String(block.kind ?? "");
  }
}

function compactAttrFragment(block) {
  switch (block.kind) {
    case "link":
      return compactHrefFragment(block.attributes?.href);
    case "input":
      return block.attributes?.inputType
        ? `=${String(block.attributes.inputType)}`
        : null;
    case "table":
      return compactCountFragment(
        block.attributes?.rows,
        block.attributes?.columns,
      );
    case "list":
      return typeof block.attributes?.items === "number"
        ? `n${block.attributes.items}`
        : null;
    case "form":
      return typeof block.attributes?.controls === "number"
        ? `n${block.attributes.controls}`
        : null;
    default:
      return null;
  }
}

function compactCountFragment(rows, columns) {
  const hasRows = typeof rows === "number";
  const hasColumns = typeof columns === "number";

  if (hasRows && hasColumns) {
    return `${rows}x${columns}`;
  }
  if (hasRows) {
    return `r${rows}`;
  }
  if (hasColumns) {
    return `c${columns}`;
  }
  return null;
}

function compactHrefFragment(href) {
  if (typeof href !== "string") {
    return null;
  }

  const trimmed = href.trim();
  if (!trimmed) {
    return null;
  }

  let externalPrefix = null;
  if (trimmed.startsWith("https://")) {
    externalPrefix = "https://";
  } else if (trimmed.startsWith("http://")) {
    externalPrefix = "http://";
  }

  if (externalPrefix) {
    const rest = trimmed.slice(externalPrefix.length);
    const host = trimTrailingSlashes(rest.split("/")[0] ?? rest);
    return host ? `@${host}` : null;
  }

  if (trimmed.startsWith("mailto:")) {
    return `@${trimmed.slice("mailto:".length)}`;
  }

  if (trimmed.startsWith("tel:")) {
    return `@${trimmed.slice("tel:".length)}`;
  }

  return null;
}

function compactTextDigest(text, kind) {
  const normalized = text.split(/\s+/).filter(Boolean);
  if (normalized.length === 0) {
    return "";
  }

  const actionable =
    kind === "heading" ||
    kind === "link" ||
    kind === "button" ||
    kind === "input";
  const limit = tokenLimitFor(kind);

  const kept = [];
  const seen = new Set();

  for (const token of normalized) {
    const cleaned = compactToken(token);
    if (!cleaned) {
      continue;
    }

    const lowered = cleaned.toLowerCase();
    if (seen.has(lowered)) {
      continue;
    }
    seen.add(lowered);

    if (actionable || isSignalToken(cleaned, lowered)) {
      kept.push(truncateCompactToken(cleaned));
    }

    if (kept.length >= limit) {
      break;
    }
  }

  if (kept.length === 0) {
    return normalized
      .map(compactToken)
      .filter(Boolean)
      .slice(0, limit)
      .map(truncateCompactToken)
      .join(" ");
  }

  return kept.join(" ");
}

function tokenLimitFor(kind) {
  switch (kind) {
    case "heading":
      return 2;
    case "link":
    case "button":
      return 1;
    case "table":
      return 2;
    default:
      return 1;
  }
}

function compactToken(token) {
  return trimCompactTokenEdges(token).trim();
}

function truncateCompactToken(token) {
  return token.length <= 12 ? token : token.slice(0, 12);
}

function isSignalToken(token, lowered) {
  return (
    containsAsciiDigit(token) ||
    token.startsWith("$") ||
    token.includes("%") ||
    token.includes("/") ||
    token.includes(":") ||
    (!isStopword(lowered) && token.length > 5)
  );
}

function isStopword(token) {
  return new Set([
    "a",
    "an",
    "and",
    "are",
    "as",
    "at",
    "be",
    "by",
    "for",
    "from",
    "in",
    "into",
    "is",
    "of",
    "on",
    "or",
    "that",
    "the",
    "this",
    "to",
    "with",
  ]).has(token);
}

function isSalientTextBlock(text) {
  const wordCount = text.split(/\s+/).filter(Boolean).length;
  const lowered = text.toLowerCase();

  return (
    wordCount <= 10 ||
    containsAsciiDigit(text) ||
    text.includes("$") ||
    text.includes("%") ||
    lowered.includes("rfc")
  );
}

function trimTrailingSlashes(value) {
  let end = value.length;
  while (end > 0 && value[end - 1] === "/") {
    end -= 1;
  }
  return value.slice(0, end);
}

function trimCompactTokenEdges(token) {
  let start = 0;
  let end = token.length;

  while (start < end && !isCompactTokenChar(token[start])) {
    start += 1;
  }

  while (end > start && !isCompactTokenChar(token[end - 1])) {
    end -= 1;
  }

  return token.slice(start, end);
}

function isCompactTokenChar(char) {
  return (
    isAsciiAlphaNumeric(char) ||
    char === "$" ||
    char === "%" ||
    char === "/" ||
    char === "." ||
    char === ":" ||
    char === "-" ||
    char === "_"
  );
}

function isAsciiAlphaNumeric(char) {
  return isAsciiLower(char) || isAsciiUpper(char) || isAsciiDigit(char);
}

function isAsciiLower(char) {
  return char >= "a" && char <= "z";
}

function isAsciiUpper(char) {
  return char >= "A" && char <= "Z";
}

function isAsciiDigit(char) {
  return char >= "0" && char <= "9";
}

function containsAsciiDigit(text) {
  for (const char of text) {
    if (isAsciiDigit(char)) {
      return true;
    }
  }
  return false;
}
