export type HtmlAttribute = {
  readonly name: string;
  readonly value?: string;
};

export type HtmlElementReplacement = {
  readonly inner: string;
  readonly attributes: ReadonlyArray<HtmlAttribute>;
};

export type HtmlTagReplacement = {
  readonly name?: string;
  readonly closing: boolean;
  readonly selfClosing: boolean;
  readonly attributes: ReadonlyArray<HtmlAttribute>;
  readonly rawTag: string;
};

export function stripHtml(input: string): string;
export function normalizeCleanedDom(input: string): string;
export function stripHtmlPreservingMarkdown(input: string): string;
export function extractFirstElementInnerHtml(
  input: string,
  tagNames: ReadonlyArray<string>,
): string | undefined;
export function replaceElementBlocks(
  input: string,
  tagName: string,
  replacer: (replacement: HtmlElementReplacement) => string,
): string;
export function replaceTags(
  input: string,
  tagNames: ReadonlyArray<string>,
  replacer: (replacement: HtmlTagReplacement) => string,
): string;
