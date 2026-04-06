import { createServer } from "node:http";
import path from "node:path";
import { fileURLToPath } from "node:url";

export {
  renderCompactSnapshot,
  renderReadingCompactSnapshot,
} from "./compact-snapshot.mjs";
import {
  normalizeCleanedDom as normalizeHtmlDom,
  stripHtml as stripHtmlTags,
} from "./html-utils.mjs";
import { spawnShell } from "./shell-command.mjs";

export const currentDir = path.dirname(fileURLToPath(import.meta.url));
export const repoRoot = path.resolve(currentDir, "../..");
const SHELL_SINGLE_QUOTE_ESCAPE = String.raw`'\''`;

export const liveSamples = [
  {
    id: "start",
    path: "/start",
    title: "Research Start",
    mustContainTexts: ["Research start page", "Docs page", "Pricing page"],
    html: `<!doctype html>
<html>
  <head><title>Research Start</title></head>
  <body>
    <main>
      <h1>Research start page</h1>
      <p>Touch Browser opens local live documents through the acquisition engine.</p>
      <nav>
        <a href="/docs">Docs page</a>
        <a href="/pricing">Pricing page</a>
      </nav>
    </main>
  </body>
</html>`,
  },
  {
    id: "docs",
    path: "/docs",
    title: "Research Docs",
    mustContainTexts: [
      "Docs page for live synthesis.",
      "Semantic snapshots keep stable refs and evidence metadata.",
      "Pricing page",
    ],
    html: `<!doctype html>
<html>
  <head><title>Research Docs</title></head>
  <body>
    <main>
      <h1>Docs page for live synthesis.</h1>
      <p>Semantic snapshots keep stable refs and evidence metadata.</p>
      <a href="/pricing">Pricing page</a>
    </main>
  </body>
</html>`,
  },
  {
    id: "pricing",
    path: "/pricing",
    title: "Research Pricing",
    mustContainTexts: [
      "Pricing page for live synthesis.",
      "Starter plan costs $29 per month.",
      "Enterprise plan requires contact with sales.",
    ],
    html: `<!doctype html>
<html>
  <head><title>Research Pricing</title></head>
  <body>
    <main>
      <h1>Pricing page for live synthesis.</h1>
      <p>Starter plan costs $29 per month.</p>
      <p>Enterprise plan requires contact with sales.</p>
      <a href="/start">Back to start</a>
    </main>
  </body>
</html>`,
  },
];

export async function withLiveSampleServer(callback) {
  const server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://127.0.0.1");

    if (url.pathname === "/robots.txt") {
      response.writeHead(200, { "content-type": "text/plain; charset=utf-8" });
      response.end("User-agent: *\nAllow: /\n");
      return;
    }

    const sample = liveSamples.find((entry) => entry.path === url.pathname);
    if (!sample) {
      response.writeHead(404, { "content-type": "text/plain; charset=utf-8" });
      response.end("not found");
      return;
    }

    response.writeHead(200, { "content-type": "text/html; charset=utf-8" });
    response.end(sample.html);
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });

  const address = server.address();
  if (!address || typeof address === "string") {
    server.close();
    throw new Error("Could not determine live sample server port.");
  }

  const baseUrl = `http://127.0.0.1:${address.port}`;

  try {
    return await callback({ baseUrl });
  } finally {
    await new Promise((resolve, reject) => {
      server.close((error) => {
        if (error) {
          reject(error);
          return;
        }

        resolve(undefined);
      });
    });
  }
}

export async function ensureCliBuilt() {
  await runShell("cargo build -q -p touch-browser-cli");
}

export async function runShell(command) {
  const child = spawnShell(command, {
    cwd: repoRoot,
    stdio: ["ignore", "pipe", "pipe"],
  });

  const stdout = [];
  const stderr = [];
  child.stdout.on("data", (chunk) => stdout.push(chunk));
  child.stderr.on("data", (chunk) => stderr.push(chunk));

  const exitCode = await new Promise((resolve, reject) => {
    child.on("error", reject);
    child.on("close", resolve);
  });

  if (exitCode !== 0) {
    throw new Error(Buffer.concat(stderr).toString("utf8"));
  }

  return Buffer.concat(stdout).toString("utf8").trim();
}

export function shellEscape(value) {
  return `'${String(value).replaceAll("'", SHELL_SINGLE_QUOTE_ESCAPE)}'`;
}

export function stripHtml(html) {
  return stripHtmlTags(html);
}

export function normalizeCleanedDom(html) {
  return normalizeHtmlDom(html);
}

export function normalizeText(input) {
  return input.trim().split(/\s+/).join(" ");
}

export function roundTo(value, digits) {
  return Number(value.toFixed(digits));
}
