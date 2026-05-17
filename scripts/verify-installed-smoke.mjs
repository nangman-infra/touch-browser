#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import {
  existsSync,
  mkdtempSync,
  readdirSync,
  rmSync,
  statSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const repoRoot = path.resolve(new URL("..", import.meta.url).pathname);
const command =
  process.argv[2] ||
  process.env.TOUCH_BROWSER_SMOKE_COMMAND ||
  resolveBundleCommand() ||
  "touch-browser";
const tempRoot = mkdtempSync(
  path.join(tmpdir(), "touch-browser-installed-smoke-"),
);
const env = {
  ...process.env,
  TOUCH_BROWSER_DATA_ROOT: path.join(tempRoot, "data"),
};

try {
  const checkoutUrl = checkoutDataUrl();

  const version = runText(["--version"]);
  assert(
    version.startsWith("touch-browser "),
    `unexpected version output: ${version}`,
  );

  const doctor = runJson(["doctor"]);
  assert(
    ["ready", "attention-required"].includes(doctor.status),
    "doctor returned unknown status",
  );
  assert(Array.isArray(doctor.checks), "doctor did not return checks");

  const status = runJson(["status"]);
  assert(status.status === "ready", "status command did not report ready");

  const quickTarget =
    process.env.TOUCH_BROWSER_INSTALLED_SMOKE_LIVE === "1"
      ? "https://www.iana.org/help/example-domains"
      : checkoutUrl;
  const quickClaim =
    process.env.TOUCH_BROWSER_INSTALLED_SMOKE_LIVE === "1"
      ? "example.com and example.org are maintained for documentation purposes."
      : "The checkout review shows an order total of $149.00.";
  const quick = runJson(["quick", quickTarget, "--claim", quickClaim]);
  const quickOutcome = quick.extract?.output?.claimOutcomes?.[0];
  assert(
    quickOutcome?.verdict === "evidence-supported",
    "quick did not return supported evidence",
  );
  assert(
    quickOutcome?.reuseAllowed === true,
    "quick did not mark the claim reusable",
  );

  const sessionFile = path.join(tempRoot, "checkout-session.json");
  const opened = runJson([
    "open",
    checkoutUrl,
    "--browser",
    "--session-file",
    sessionFile,
  ]);
  assert(opened.status === "succeeded", "browser-backed checkout open failed");
  const buttonRef = opened.output?.blocks?.find(
    (block) =>
      block.kind === "button" &&
      /confirm|purchase|payment/i.test(block.text ?? ""),
  )?.ref;
  assert(buttonRef, "checkout button ref was not captured");

  const rejected = runRaw([
    "submit",
    "--session-file",
    sessionFile,
    "--ref",
    buttonRef,
  ]);
  assert(
    rejected.status !== 0,
    "rejected high-risk submit exited with status 0",
  );
  const rejectedJson = parseJson(rejected.stdout, "rejected submit stdout");
  const action =
    rejectedJson.action ?? rejectedJson.result?.action ?? rejectedJson.result;
  assert(
    action?.status === "rejected",
    "high-risk submit did not return rejected action status",
  );
  assert(
    action?.failureKind === "policy-blocked",
    "high-risk submit did not return policy-blocked",
  );

  console.log(
    JSON.stringify(
      {
        status: "ok",
        command,
        version,
        doctorStatus: doctor.status,
        quickVerdict: quickOutcome.verdict,
        rejectedExitCode: rejected.status,
      },
      null,
      2,
    ),
  );
} finally {
  rmSync(tempRoot, { recursive: true, force: true });
}

function resolveBundleCommand() {
  const standaloneRoot = path.join(repoRoot, "dist", "standalone");
  try {
    const candidates = readdirSync(standaloneRoot, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) =>
        path.join(standaloneRoot, entry.name, "bin", "touch-browser"),
      )
      .filter((candidate) => existsSync(candidate))
      .map((candidate) => ({
        path: candidate,
        mtimeMs: statSync(candidate).mtimeMs,
      }))
      .sort((left, right) => left.mtimeMs - right.mtimeMs);
    return candidates.at(-1)?.path;
  } catch {
    return null;
  }
}

function checkoutDataUrl() {
  const html = `<!doctype html>
      <html>
        <head><title>Installed Smoke Checkout</title></head>
        <body>
          <main>
            <h1>Checkout review</h1>
            <p>The checkout review shows an order total of $149.00.</p>
            <form id="checkout-form">
              <input name="amount" value="$149.00" />
              <button type="submit">Confirm purchase</button>
            </form>
          </main>
        </body>
      </html>`;
  return `data:text/html;charset=utf-8,${encodeURIComponent(html)}`;
}

function runText(args) {
  const result = runRaw(args);
  if (result.status !== 0) {
    throw new Error(
      `command failed: ${command} ${args.join(" ")}\n${result.stderr}`,
    );
  }
  return result.stdout.trim();
}

function runJson(args) {
  return parseJson(runText(args), `${args[0]} stdout`);
}

function runRaw(args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    env,
    encoding: "utf8",
  });
  return {
    status: result.status ?? 1,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function parseJson(text, label) {
  try {
    return JSON.parse(text);
  } catch (error) {
    throw new Error(
      `could not parse ${label} as JSON: ${error.message}\n${text}`,
    );
  }
}

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}
