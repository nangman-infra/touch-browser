const RAW_TEXT_TAGS = new Set(["script", "style", "noscript", "svg"]);
const NORMALIZED_ATTR_NAMES = new Set([
  "href",
  "src",
  "role",
  "type",
  "name",
  "title",
  "hidden",
]);

export function stripHtml(input) {
  return transformHtml(input, (token) => stripHtmlToken(input, token));
}

export function normalizeCleanedDom(input) {
  const output = transformHtml(input, (token) =>
    normalizeCleanedDomToken(input, token),
  );
  return collapseMarkupWhitespace(output);
}

export function stripHtmlPreservingMarkdown(input) {
  return transformHtml(input, (token) =>
    stripHtmlPreservingMarkdownToken(input, token),
  );
}

export function extractFirstElementInnerHtml(input, tagNames) {
  for (const tagName of tagNames) {
    const start = findOpeningTag(input, tagName, 0);
    if (start === -1) {
      continue;
    }

    const startToken = readTagToken(input, start);
    if (!startToken) {
      continue;
    }

    const closing = findMatchingElementClose(
      input,
      tagName,
      startToken.nextCursor,
    );
    if (!closing) {
      continue;
    }

    return input.slice(startToken.nextCursor, closing.start);
  }

  return undefined;
}

export function replaceElementBlocks(input, tagName, replacer) {
  let output = "";
  let cursor = 0;
  let searchFrom = 0;

  while (searchFrom < input.length) {
    const start = findOpeningTag(input, tagName, searchFrom);
    if (start === -1) {
      output += input.slice(cursor);
      break;
    }

    const startToken = readTagToken(input, start);
    if (!startToken) {
      output += input.slice(cursor);
      break;
    }

    if (!startToken.descriptor.name || startToken.descriptor.closing) {
      searchFrom = startToken.nextCursor;
      continue;
    }

    const closing = findMatchingElementClose(
      input,
      tagName,
      startToken.nextCursor,
    );
    if (!closing) {
      output += input.slice(cursor, startToken.nextCursor);
      cursor = startToken.nextCursor;
      searchFrom = cursor;
      continue;
    }

    output += input.slice(cursor, start);
    output += replacer({
      inner: input.slice(startToken.nextCursor, closing.start),
      attributes: parseAttributes(startToken.rawTag),
    });
    cursor = closing.end + 1;
    searchFrom = cursor;
  }

  return output;
}

export function replaceTags(input, tagNames, replacer) {
  const normalizedTagNames = new Set(
    tagNames.map((tagName) => String(tagName).toLowerCase()),
  );
  let output = "";
  let cursor = 0;

  while (cursor < input.length) {
    const token = readNextHtmlToken(input, cursor);
    output += token.leadingText;

    if (token.kind === "trailing") {
      break;
    }

    if (token.kind === "comment") {
      output += token.rawTag;
      cursor = token.nextCursor;
      continue;
    }

    if (token.kind === "unterminated") {
      output += token.rawTag;
      break;
    }

    if (
      !token.descriptor.name ||
      !normalizedTagNames.has(token.descriptor.name)
    ) {
      output += token.rawTag;
      cursor = token.nextCursor;
      continue;
    }

    output += replacer({
      ...token.descriptor,
      attributes: parseAttributes(token.rawTag),
      rawTag: token.rawTag,
    });
    cursor = token.nextCursor;
  }

  return output;
}

function transformHtml(input, transformToken) {
  let output = "";
  let cursor = 0;

  while (cursor < input.length) {
    const token = readNextHtmlToken(input, cursor);
    output += token.leadingText;

    if (token.kind === "trailing") {
      break;
    }

    const result = transformToken(token);
    output += result.append;
    cursor = result.nextCursor;
  }

  return output;
}

function stripHtmlToken(input, token) {
  if (token.kind === "comment") {
    return createTransformStep(" ", token.nextCursor);
  }

  if (token.kind === "unterminated") {
    return createTransformStep(token.rawTag, token.nextCursor);
  }

  if (!token.descriptor.name) {
    return createTransformStep(token.rawTag, token.nextCursor);
  }

  return createSpaceStep(input, token);
}

function normalizeCleanedDomToken(input, token) {
  if (token.kind === "comment") {
    return createTransformStep("", token.nextCursor);
  }

  if (token.kind === "unterminated") {
    return createTransformStep(token.rawTag, token.nextCursor);
  }

  if (!token.descriptor.name) {
    return createTransformStep(token.rawTag, token.nextCursor);
  }

  if (shouldSkipRawText(token.descriptor)) {
    return createTransformStep("", skipRawTextBlock(input, token));
  }

  if (token.descriptor.closing) {
    return createTransformStep(`</${token.descriptor.name}>`, token.nextCursor);
  }

  return createTransformStep(renderNormalizedOpenTag(token), token.nextCursor);
}

function stripHtmlPreservingMarkdownToken(input, token) {
  if (token.kind === "comment") {
    return createTransformStep(" ", token.nextCursor);
  }

  if (token.kind === "unterminated") {
    return createTransformStep(token.rawTag, token.nextCursor);
  }

  if (isPreservedMarkdownTag(token.rawTag)) {
    return createTransformStep(token.rawTag, token.nextCursor);
  }

  if (!token.descriptor.name) {
    return createTransformStep(" ", token.nextCursor);
  }

  return createSpaceStep(input, token);
}

function createSpaceStep(input, token) {
  const nextCursor = shouldSkipRawText(token.descriptor)
    ? skipRawTextBlock(input, token)
    : token.nextCursor;
  return createTransformStep(" ", nextCursor);
}

function createTransformStep(append, nextCursor) {
  return { append, nextCursor };
}

function readNextHtmlToken(input, cursor) {
  const tagStart = input.indexOf("<", cursor);
  if (tagStart === -1) {
    return {
      kind: "trailing",
      leadingText: input.slice(cursor),
      nextCursor: input.length,
    };
  }

  const leadingText = input.slice(cursor, tagStart);
  if (input.startsWith("<!--", tagStart)) {
    return readCommentToken(input, tagStart, leadingText);
  }

  const tagToken = readTagToken(input, tagStart);
  if (!tagToken) {
    return {
      kind: "unterminated",
      leadingText,
      rawTag: input.slice(tagStart),
      nextCursor: input.length,
    };
  }

  return {
    kind: "tag",
    leadingText,
    ...tagToken,
  };
}

function readCommentToken(input, tagStart, leadingText) {
  const commentEnd = input.indexOf("-->", tagStart + 4);
  const nextCursor = commentEnd === -1 ? input.length : commentEnd + 3;
  return {
    kind: "comment",
    leadingText,
    rawTag: input.slice(tagStart, nextCursor),
    nextCursor,
  };
}

function readTagToken(input, tagStart) {
  const tagEnd = findTagEnd(input, tagStart);
  if (tagEnd === -1) {
    return null;
  }

  const rawTag = input.slice(tagStart, tagEnd + 1);
  return {
    rawTag,
    descriptor: parseTag(rawTag),
    tagEnd,
    nextCursor: tagEnd + 1,
  };
}

function skipRawTextBlock(input, token) {
  const closing = findMatchingElementClose(
    input,
    token.descriptor.name,
    token.nextCursor,
  );
  return closing ? closing.end + 1 : input.length;
}

function shouldSkipRawText(descriptor) {
  return !descriptor.closing && RAW_TEXT_TAGS.has(descriptor.name);
}

function isPreservedMarkdownTag(rawTag) {
  const preserveMarkerIndex = rawTag[1] === "/" ? 2 : 1;
  const preserveMarker = rawTag[preserveMarkerIndex];
  return (
    preserveMarker === "#" || preserveMarker === "[" || preserveMarker === "]"
  );
}

function renderNormalizedOpenTag(token) {
  const keptAttrs = parseAttributes(token.rawTag)
    .filter(({ name }) => shouldKeepNormalizedAttribute(name))
    .map(renderNormalizedAttribute);
  return keptAttrs.length > 0
    ? `<${token.descriptor.name} ${keptAttrs.join(" ")}>`
    : `<${token.descriptor.name}>`;
}

function shouldKeepNormalizedAttribute(name) {
  return NORMALIZED_ATTR_NAMES.has(name) || name.startsWith("aria-");
}

function renderNormalizedAttribute({ name, value }) {
  return value === undefined ? name : `${name}="${String(value).trim()}"`;
}

function collapseMarkupWhitespace(output) {
  return output.replaceAll(/\s+/g, " ").replaceAll(/>\s+</g, "><").trim();
}

function findOpeningTag(input, tagName, searchFrom) {
  const lower = input.toLowerCase();
  const needle = `<${tagName}`;
  let index = lower.indexOf(needle, searchFrom);

  while (index !== -1) {
    if (isTagBoundaryCharacter(lower[index + needle.length])) {
      return index;
    }
    index = lower.indexOf(needle, index + needle.length);
  }

  return -1;
}

function findMatchingElementClose(input, tagName, searchFrom) {
  const lower = input.toLowerCase();
  const closeNeedle = `</${tagName}`;
  let cursor = searchFrom;
  let depth = 1;

  while (cursor < input.length) {
    const nextClose = lower.indexOf(closeNeedle, cursor);
    if (nextClose === -1) {
      return null;
    }

    const nestedOpen = readNestedOpen(input, tagName, cursor, nextClose);
    if (nestedOpen) {
      depth += nestedOpen.depthDelta;
      cursor = nestedOpen.nextCursor;
      continue;
    }

    const closingTag = readTagToken(input, nextClose);
    if (!closingTag) {
      return null;
    }

    depth -= 1;
    if (depth === 0) {
      return { start: nextClose, end: closingTag.tagEnd };
    }
    cursor = closingTag.nextCursor;
  }

  return null;
}

function readNestedOpen(input, tagName, cursor, nextClose) {
  const nextOpen = findOpeningTag(input, tagName, cursor);
  if (nextOpen === -1 || nextOpen >= nextClose) {
    return null;
  }

  const openTag = readTagToken(input, nextOpen);
  if (!openTag) {
    return null;
  }

  return {
    depthDelta: openTag.descriptor.selfClosing ? 0 : 1,
    nextCursor: openTag.nextCursor,
  };
}

function findTagEnd(input, tagStart) {
  let quote = null;

  for (let index = tagStart + 1; index < input.length; index += 1) {
    const character = input[index];
    if (quote) {
      if (character === quote) {
        quote = null;
      }
      continue;
    }
    if (character === '"' || character === "'") {
      quote = character;
      continue;
    }
    if (character === ">") {
      return index;
    }
  }

  return -1;
}

function parseTag(rawTag) {
  let cursor = skipWhitespace(rawTag, 1);
  const closing = rawTag[cursor] === "/";
  if (closing) {
    cursor += 1;
  }

  cursor = skipWhitespace(rawTag, cursor);
  const nameStart = cursor;
  while (cursor < rawTag.length && isTagNameCharacter(rawTag[cursor])) {
    cursor += 1;
  }

  const name = rawTag.slice(nameStart, cursor).toLowerCase();
  return {
    closing,
    name: name || undefined,
    selfClosing: /\/\s*>$/.test(rawTag),
  };
}

function parseAttributes(rawTag) {
  const attributes = [];
  let cursor = skipWhitespace(rawTag, 1);

  if (rawTag[cursor] === "/") {
    return attributes;
  }

  cursor = skipTagName(rawTag, cursor);

  while (cursor < rawTag.length) {
    cursor = skipWhitespace(rawTag, cursor);
    if (isAttributeListEnd(rawTag, cursor)) {
      break;
    }

    const nextAttribute = readAttribute(rawTag, cursor);
    cursor = nextAttribute.nextCursor;
    if (nextAttribute.attribute) {
      attributes.push(nextAttribute.attribute);
    }
  }

  return attributes;
}

function readAttribute(rawTag, cursor) {
  const nameResult = readAttributeName(rawTag, cursor);
  if (!nameResult.name) {
    return { nextCursor: nameResult.nextCursor };
  }

  let nextCursor = skipWhitespace(rawTag, nameResult.nextCursor);
  if (rawTag[nextCursor] !== "=") {
    return {
      attribute: { name: nameResult.name },
      nextCursor,
    };
  }

  nextCursor += 1;
  nextCursor = skipWhitespace(rawTag, nextCursor);
  const valueResult = readAttributeValue(rawTag, nextCursor);
  return {
    attribute: { name: nameResult.name, value: valueResult.value },
    nextCursor: valueResult.nextCursor,
  };
}

function readAttributeName(rawTag, cursor) {
  const nameStart = cursor;
  let nextCursor = cursor;
  while (
    nextCursor < rawTag.length &&
    isAttributeNameCharacter(rawTag[nextCursor])
  ) {
    nextCursor += 1;
  }

  return {
    name: rawTag.slice(nameStart, nextCursor).toLowerCase(),
    nextCursor,
  };
}

function readAttributeValue(rawTag, cursor) {
  const quote = rawTag[cursor];
  if (quote === '"' || quote === "'") {
    return readQuotedAttributeValue(rawTag, cursor, quote);
  }

  return readUnquotedAttributeValue(rawTag, cursor);
}

function readQuotedAttributeValue(rawTag, cursor, quote) {
  const valueStart = cursor + 1;
  let nextCursor = valueStart;

  while (nextCursor < rawTag.length && rawTag[nextCursor] !== quote) {
    nextCursor += 1;
  }

  return {
    value: rawTag.slice(valueStart, nextCursor),
    nextCursor: rawTag[nextCursor] === quote ? nextCursor + 1 : nextCursor,
  };
}

function readUnquotedAttributeValue(rawTag, cursor) {
  const valueStart = cursor;
  let nextCursor = cursor;

  while (
    nextCursor < rawTag.length &&
    !isAttributeValueBoundary(rawTag[nextCursor])
  ) {
    nextCursor += 1;
  }

  return {
    value: rawTag.slice(valueStart, nextCursor),
    nextCursor,
  };
}

function skipWhitespace(input, cursor) {
  let nextCursor = cursor;
  while (nextCursor < input.length && /\s/.test(input[nextCursor] ?? "")) {
    nextCursor += 1;
  }
  return nextCursor;
}

function skipTagName(rawTag, cursor) {
  let nextCursor = cursor;
  while (nextCursor < rawTag.length && isTagNameCharacter(rawTag[nextCursor])) {
    nextCursor += 1;
  }
  return nextCursor;
}

function isTagBoundaryCharacter(character) {
  return (
    character === undefined ||
    character === ">" ||
    character === "/" ||
    /\s/.test(character)
  );
}

function isAttributeNameCharacter(character) {
  return !isAttributeListEndCharacter(character) && character !== "=";
}

function isAttributeListEnd(rawTag, cursor) {
  return isAttributeListEndCharacter(rawTag[cursor]);
}

function isAttributeListEndCharacter(character) {
  return !character || character === ">" || character === "/";
}

function isAttributeValueBoundary(character) {
  return (
    character === undefined ||
    character === "/" ||
    character === ">" ||
    /\s/.test(character)
  );
}

function isTagNameCharacter(character) {
  return /[A-Za-z0-9:-]/.test(character ?? "");
}
