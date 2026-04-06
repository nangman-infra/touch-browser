export const currentDir: string;
export const repoRoot: string;

export function withLiveSampleServer<T>(
  callback: (context: { readonly baseUrl: string }) => Promise<T> | T,
): Promise<T>;
export function ensureCliBuilt(): Promise<void>;
export function runShell(command: string): Promise<string>;
export function shellEscape(value: unknown): string;
export function stripHtml(html: string): string;
export function normalizeCleanedDom(html: string): string;
export function normalizeText(input: string): string;
export function roundTo(value: number, digits: number): number;
