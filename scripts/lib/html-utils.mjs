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
  let output = "";
  let cursor = 0;

  while (cursor < input.length) {
    const tagStart = input.indexOf("<", cursor);
    if (tagStart === -1) {
      output += input.slice(cursor);
      break;
    }

    output += input.slice(cursor, tagStart);

    if (input.startsWith("<!--", tagStart)) {
      const commentEnd = input.indexOf("-->", tagStart + 4);
      output += " ";
      cursor = commentEnd === -1 ? input.length : commentEnd + 3;
      continue;
    }

    const tagEnd = findTagEnd(input, tagStart);
    if (tagEnd === -1) {
      output += input.slice(tagStart);
      break;
    }

    const rawTag = input.slice(tagStart, tagEnd + 1);
    const descriptor = parseTag(rawTag);
    if (!descriptor.name) {
      output += rawTag;
      cursor = tagEnd + 1;
      continue;
    }

    output += " ";
    if (!descriptor.closing && RAW_TEXT_TAGS.has(descriptor.name)) {
      const closing = findMatchingElementClose(
        input,
        descriptor.name,
        tagEnd + 1,
      );
      cursor = closing ? closing.end + 1 : input.length;
      continue;
    }

    cursor = tagEnd + 1;
  }

  return output;
}

export function normalizeCleanedDom(input) {
  let output = "";
  let cursor = 0;

  while (cursor < input.length) {
    const tagStart = input.indexOf("<", cursor);
    if (tagStart === -1) {
      output += input.slice(cursor);
      break;
    }

    output += input.slice(cursor, tagStart);

    if (input.startsWith("<!--", tagStart)) {
      const commentEnd = input.indexOf("-->", tagStart + 4);
      cursor = commentEnd === -1 ? input.length : commentEnd + 3;
      continue;
    }

    const tagEnd = findTagEnd(input, tagStart);
    if (tagEnd === -1) {
      output += input.slice(tagStart);
      break;
    }

    const rawTag = input.slice(tagStart, tagEnd + 1);
    const descriptor = parseTag(rawTag);
    if (!descriptor.name) {
      output += rawTag;
      cursor = tagEnd + 1;
      continue;
    }

    if (!descriptor.closing && RAW_TEXT_TAGS.has(descriptor.name)) {
      const closing = findMatchingElementClose(
        input,
        descriptor.name,
        tagEnd + 1,
      );
      cursor = closing ? closing.end + 1 : input.length;
      continue;
    }

    if (descriptor.closing) {
      output += `</${descriptor.name}>`;
      cursor = tagEnd + 1;
      continue;
    }

    const keptAttrs = parseAttributes(rawTag)
      .filter(
        ({ name }) =>
          NORMALIZED_ATTR_NAMES.has(name) || name.startsWith("aria-"),
      )
      .map(({ name, value }) =>
        value === undefined ? name : `${name}="${String(value).trim()}"`,
      );
    output +=
      keptAttrs.length > 0
        ? `<${descriptor.name} ${keptAttrs.join(" ")}>`
        : `<${descriptor.name}>`;
    cursor = tagEnd + 1;
  }

  return output.replace(/\s+/g, " ").replace(/>\s+</g, "><").trim();
}

export function stripHtmlPreservingMarkdown(input) {
  let output = "";
  let cursor = 0;

  while (cursor < input.length) {
    const tagStart = input.indexOf("<", cursor);
    if (tagStart === -1) {
      output += input.slice(cursor);
      break;
    }

    output += input.slice(cursor, tagStart);

    if (input.startsWith("<!--", tagStart)) {
      const commentEnd = input.indexOf("-->", tagStart + 4);
      output += " ";
      cursor = commentEnd === -1 ? input.length : commentEnd + 3;
      continue;
    }

    const tagEnd = findTagEnd(input, tagStart);
    if (tagEnd === -1) {
      output += input.slice(tagStart);
      break;
    }

    const rawTag = input.slice(tagStart, tagEnd + 1);
    const preserveMarkerIndex = rawTag[1] === "/" ? 2 : 1;
    const preserveMarker = rawTag[preserveMarkerIndex];
    if (
      preserveMarker === "#" ||
      preserveMarker === "[" ||
      preserveMarker === "]"
    ) {
      output += rawTag;
      cursor = tagEnd + 1;
      continue;
    }

    const descriptor = parseTag(rawTag);
    if (!descriptor.name) {
      output += " ";
      cursor = tagEnd + 1;
      continue;
    }

    output += " ";
    if (!descriptor.closing && RAW_TEXT_TAGS.has(descriptor.name)) {
      const closing = findMatchingElementClose(
        input,
        descriptor.name,
        tagEnd + 1,
      );
      cursor = closing ? closing.end + 1 : input.length;
      continue;
    }

    cursor = tagEnd + 1;
  }

  return output;
}

export function extractFirstElementInnerHtml(input, tagNames) {
  for (const tagName of tagNames) {
    const start = findOpeningTag(input, tagName, 0);
    if (start === -1) {
      continue;
    }

    const startEnd = findTagEnd(input, start);
    if (startEnd === -1) {
      continue;
    }

    const closing = findMatchingElementClose(input, tagName, startEnd + 1);
    if (!closing) {
      continue;
    }

    return input.slice(startEnd + 1, closing.start);
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

    const startEnd = findTagEnd(input, start);
    if (startEnd === -1) {
      output += input.slice(cursor);
      break;
    }

    const rawStartTag = input.slice(start, startEnd + 1);
    const descriptor = parseTag(rawStartTag);
    if (!descriptor.name || descriptor.closing) {
      searchFrom = startEnd + 1;
      continue;
    }

    const closing = findMatchingElementClose(input, tagName, startEnd + 1);
    if (!closing) {
      output += input.slice(cursor, startEnd + 1);
      cursor = startEnd + 1;
      searchFrom = cursor;
      continue;
    }

    output += input.slice(cursor, start);
    output += replacer({
      inner: input.slice(startEnd + 1, closing.start),
      attributes: parseAttributes(rawStartTag),
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
    const tagStart = input.indexOf("<", cursor);
    if (tagStart === -1) {
      output += input.slice(cursor);
      break;
    }

    output += input.slice(cursor, tagStart);

    if (input.startsWith("<!--", tagStart)) {
      const commentEnd = input.indexOf("-->", tagStart + 4);
      if (commentEnd === -1) {
        output += input.slice(tagStart);
        break;
      }
      output += input.slice(tagStart, commentEnd + 3);
      cursor = commentEnd + 3;
      continue;
    }

    const tagEnd = findTagEnd(input, tagStart);
    if (tagEnd === -1) {
      output += input.slice(tagStart);
      break;
    }

    const rawTag = input.slice(tagStart, tagEnd + 1);
    const descriptor = parseTag(rawTag);
    if (!descriptor.name || !normalizedTagNames.has(descriptor.name)) {
      output += rawTag;
      cursor = tagEnd + 1;
      continue;
    }

    output += replacer({
      ...descriptor,
      attributes: parseAttributes(rawTag),
      rawTag,
    });
    cursor = tagEnd + 1;
  }

  return output;
}

function findOpeningTag(input, tagName, searchFrom) {
  const lower = input.toLowerCase();
  const needle = `<${tagName}`;
  let index = lower.indexOf(needle, searchFrom);

  while (index !== -1) {
    const boundary = lower[index + needle.length];
    if (
      boundary === undefined ||
      boundary === ">" ||
      boundary === "/" ||
      /\s/.test(boundary)
    ) {
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
    const nextOpen = findOpeningTag(input, tagName, cursor);
    const nextClose = lower.indexOf(closeNeedle, cursor);
    if (nextClose === -1) {
      return null;
    }

    if (nextOpen !== -1 && nextOpen < nextClose) {
      const openEnd = findTagEnd(input, nextOpen);
      if (openEnd === -1) {
        return null;
      }
      const descriptor = parseTag(input.slice(nextOpen, openEnd + 1));
      if (!descriptor.selfClosing) {
        depth += 1;
      }
      cursor = openEnd + 1;
      continue;
    }

    const closeEnd = findTagEnd(input, nextClose);
    if (closeEnd === -1) {
      return null;
    }

    depth -= 1;
    if (depth === 0) {
      return { start: nextClose, end: closeEnd };
    }
    cursor = closeEnd + 1;
  }

  return null;
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
  let cursor = 1;
  while (cursor < rawTag.length && /\s/.test(rawTag[cursor] ?? "")) {
    cursor += 1;
  }

  const closing = rawTag[cursor] === "/";
  if (closing) {
    cursor += 1;
  }

  while (cursor < rawTag.length && /\s/.test(rawTag[cursor] ?? "")) {
    cursor += 1;
  }

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
  let cursor = 1;

  while (cursor < rawTag.length && /\s/.test(rawTag[cursor] ?? "")) {
    cursor += 1;
  }
  if (rawTag[cursor] === "/") {
    return attributes;
  }

  while (cursor < rawTag.length && isTagNameCharacter(rawTag[cursor])) {
    cursor += 1;
  }

  while (cursor < rawTag.length) {
    while (cursor < rawTag.length && /\s/.test(rawTag[cursor] ?? "")) {
      cursor += 1;
    }

    const character = rawTag[cursor];
    if (!character || character === ">" || character === "/") {
      break;
    }

    const nameStart = cursor;
    while (
      cursor < rawTag.length &&
      !/\s/.test(rawTag[cursor] ?? "") &&
      !["=", "/", ">"].includes(rawTag[cursor] ?? "")
    ) {
      cursor += 1;
    }

    const name = rawTag.slice(nameStart, cursor).toLowerCase();
    while (cursor < rawTag.length && /\s/.test(rawTag[cursor] ?? "")) {
      cursor += 1;
    }

    let value;
    if (rawTag[cursor] === "=") {
      cursor += 1;
      while (cursor < rawTag.length && /\s/.test(rawTag[cursor] ?? "")) {
        cursor += 1;
      }

      const quote = rawTag[cursor];
      if (quote === '"' || quote === "'") {
        cursor += 1;
        const valueStart = cursor;
        while (cursor < rawTag.length && rawTag[cursor] !== quote) {
          cursor += 1;
        }
        value = rawTag.slice(valueStart, cursor);
        if (rawTag[cursor] === quote) {
          cursor += 1;
        }
      } else {
        const valueStart = cursor;
        while (
          cursor < rawTag.length &&
          !/\s/.test(rawTag[cursor] ?? "") &&
          !["/", ">"].includes(rawTag[cursor] ?? "")
        ) {
          cursor += 1;
        }
        value = rawTag.slice(valueStart, cursor);
      }
    }

    if (name) {
      attributes.push({ name, value });
    }
  }

  return attributes;
}

function isTagNameCharacter(character) {
  return /[A-Za-z0-9:-]/.test(character ?? "");
}
