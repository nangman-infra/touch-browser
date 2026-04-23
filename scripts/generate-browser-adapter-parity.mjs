import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { pathToFileURL } from "node:url";

const repoRoot = path.resolve(import.meta.dirname, "..");
const outputDir = path.join(repoRoot, "output", "browser-adapter-parity");
const binary = path.join(repoRoot, "target", "debug", "touch-browser");

function resolveCargoExecutable() {
  const candidates = [
    process.env.CARGO,
    process.env.CARGO_HOME
      ? path.join(process.env.CARGO_HOME, "bin", "cargo")
      : null,
    path.join(os.homedir(), ".cargo", "bin", "cargo"),
    "/usr/local/bin/cargo",
    "/opt/homebrew/bin/cargo",
    "/usr/bin/cargo",
  ];

  for (const candidate of candidates) {
    if (candidate && path.isAbsolute(candidate) && fs.existsSync(candidate)) {
      return candidate;
    }
  }

  throw new Error(
    "Could not resolve an absolute cargo path. Set CARGO to an absolute executable path.",
  );
}

const cases = [
  {
    name: "snapshot-static-docs",
    target: "fixture://research/static-docs/getting-started",
    mode: "open",
    mustContain: ["Getting Started"],
  },
  {
    name: "follow-inline-anchor",
    target: "fixture://research/navigation/browser-follow",
    mode: "action",
    action: ["follow", "--ref", "rmain:link:advanced"],
    mustContain: ["Advanced guide opened for the next research step."],
  },
  {
    name: "click-expand-details",
    target: "fixture://research/navigation/browser-expand",
    mode: "action",
    allowDomain: "research",
    action: ["click", "--ref", "rmain:button:expand-details"],
    mustContain: ["Expanded details confirm"],
  },
  {
    name: "type-login-email",
    target: "fixture://research/navigation/browser-login-form",
    mode: "action",
    allowDomain: "research",
    action: [
      "type",
      "--ref",
      "rmain:input:email-email-agent-example-com",
      "--value",
      "agent@example.com",
      "--ack-risk",
      "auth",
    ],
    mustContain: ["agent@example.com"],
  },
  {
    name: "submit-login-form",
    target: "fixture://research/navigation/browser-login-form",
    mode: "multi-action",
    allowDomain: "research",
    actions: [
      [
        "type",
        "--ref",
        "rmain:input:email-email-agent-example-com",
        "--value",
        "agent@example.com",
        "--ack-risk",
        "auth",
      ],
      ["submit", "--ref", "rmain:button:sign-in", "--ack-risk", "auth"],
    ],
    mustContain: ["Signed in draft session ready for review."],
  },
  {
    name: "paginate-next",
    target: "fixture://research/navigation/browser-pagination",
    mode: "action",
    action: ["paginate", "--direction", "next"],
    mustContain: ["Page 2 reveals"],
  },
  {
    name: "expand-details",
    target: "fixture://research/navigation/browser-expand",
    mode: "action",
    action: ["expand", "--ref", "rmain:button:expand-details"],
    mustContain: ["Expanded details confirm"],
  },
  {
    name: "iframe-same-origin-click",
    mode: "action",
    cdpOnly: true,
    allowDomain: "local-file",
    html: `<!doctype html>
<html><head><title>Iframe parity</title></head>
<body>
  <h1>Iframe test host</h1>
  <iframe title="Inline frame" srcdoc="
    <button onclick='document.body.insertAdjacentHTML(&quot;beforeend&quot;, &quot;<p>Iframe detail loaded.</p>&quot;)'>Reveal iframe detail</button>
  "></iframe>
</body></html>`,
    action: ["click", "--ref-text", "Reveal iframe detail"],
    mustContain: ["Iframe detail loaded."],
  },
  {
    name: "shadow-dom-click",
    mode: "action",
    cdpOnly: true,
    allowDomain: "local-file",
    html: `<!doctype html>
<html><head><title>Shadow parity</title></head>
<body>
  <h1>Shadow test host</h1>
  <shadow-panel></shadow-panel>
  <script>
    customElements.define("shadow-panel", class extends HTMLElement {
      connectedCallback() {
        const root = this.attachShadow({ mode: "open" });
        root.innerHTML = "<button>Show shadow detail</button><p>Shadow idle.</p>";
        root.querySelector("button").addEventListener("click", () => {
          root.querySelector("p").textContent = "Shadow detail loaded.";
        });
      }
    });
  </script>
</body></html>`,
    action: ["click", "--ref-text", "Show shadow detail"],
    mustContain: ["Shadow detail loaded."],
  },
  {
    name: "spa-navigation-click",
    mode: "action",
    allowDomain: "local-file",
    html: `<!doctype html>
<html><head><title>SPA parity</title></head>
<body>
  <main id="app">
    <h1>SPA start</h1>
    <button id="go">Open SPA result</button>
  </main>
  <script>
    document.getElementById("go").addEventListener("click", () => {
      history.pushState({ page: 2 }, "", "#result");
      setTimeout(() => {
        document.getElementById("app").innerHTML = "<h1>SPA result page</h1><p>SPA navigation settled.</p>";
      }, 50);
    });
  </script>
</body></html>`,
    action: ["click", "--ref-text", "Open SPA result"],
    mustContain: ["SPA navigation settled."],
  },
  {
    name: "download-link-click",
    mode: "action",
    allowDomain: "local-file",
    html: `<!doctype html>
<html><head><title>Download parity</title></head>
<body>
  <h1>Download test</h1>
  <a download="touch-browser-report.txt" href="data:text/plain,download-ok" onclick="document.body.insertAdjacentHTML('beforeend', '<p>Download click observed.</p>')">Download report</a>
</body></html>`,
    action: ["click", "--ref-text", "Download report"],
    mustContain: ["Download click observed."],
  },
  {
    name: "persistent-session-type",
    mode: "multi-action",
    allowDomain: "local-file",
    html: `<!doctype html>
<html><head><title>Persistent session parity</title></head>
<body>
  <form onsubmit="event.preventDefault(); document.getElementById('status').textContent = document.querySelector('input').value;">
    <label>Session note <input name="note" placeholder="Session note"></label>
    <button type="submit">Save note</button>
  </form>
  <p id="status">Waiting.</p>
</body></html>`,
    actions: [
      [
        "type",
        "--ref-text",
        "Session note",
        "--ref-kind",
        "input",
        "--value",
        "persisted note",
      ],
      ["submit", "--ref-text", "Save note"],
    ],
    mustContain: ["persisted note"],
  },
];

fs.mkdirSync(outputDir, { recursive: true });
execFileSync(resolveCargoExecutable(), ["build", "-p", "touch-browser-cli"], {
  cwd: repoRoot,
  stdio: "inherit",
});

const report = {
  generatedAt: new Date().toISOString(),
  adapters: ["playwright", "cdp-rust"],
  cases: [],
};

for (const testCase of cases) {
  const playwright = runCase(testCase, "playwright");
  const cdpRust = runCase(testCase, "cdp-rust");
  report.cases.push({
    name: testCase.name,
    mode: testCase.mode,
    playwright,
    cdpRust,
    parity: summarizeParity(testCase, playwright, cdpRust),
  });
}

const reportPath = path.join(outputDir, "report.json");
fs.writeFileSync(reportPath, `${JSON.stringify(report, null, 2)}\n`);
console.log(`Browser adapter parity report written to ${reportPath}`);

function runCase(testCase, adapter) {
  const tempRoot = fs.mkdtempSync(
    path.join(os.tmpdir(), `touch-browser-${adapter}-`),
  );
  const sessionFile = path.join(tempRoot, `${testCase.name}.json`);
  const target = materializeTarget(testCase, tempRoot);
  const env = {
    ...process.env,
    TOUCH_BROWSER_REPO_ROOT: repoRoot,
  };
  if (adapter === "cdp-rust") {
    env.TOUCH_BROWSER_BROWSER_ADAPTER = "cdp-rust";
  } else {
    env.TOUCH_BROWSER_BROWSER_ADAPTER = undefined;
  }

  try {
    const openArgs = [
      "open",
      target,
      "--browser",
      "--session-file",
      sessionFile,
    ];
    if (testCase.allowDomain) {
      openArgs.push("--allow-domain", testCase.allowDomain);
    }
    const open = runTouchBrowser(openArgs, env);
    let result = open;
    const actions =
      testCase.actions ?? (testCase.action ? [testCase.action] : []);
    for (const action of actions) {
      const resolvedAction = resolveAction(action, summarizeOutput(result));
      result = runTouchBrowser(
        [
          resolvedAction[0],
          "--session-file",
          sessionFile,
          ...resolvedAction.slice(1),
        ],
        env,
      );
    }
    return summarizeOutput(result);
  } catch (error) {
    return {
      status: "failed",
      message: error instanceof Error ? error.message : String(error),
    };
  } finally {
    fs.rmSync(tempRoot, { recursive: true, force: true });
  }
}

function materializeTarget(testCase, tempRoot) {
  if (!testCase.html) {
    return testCase.target;
  }
  const htmlPath = path.join(tempRoot, `${testCase.name}.html`);
  fs.writeFileSync(htmlPath, testCase.html);
  return pathToFileURL(htmlPath).href;
}

function runTouchBrowser(args, env) {
  const stdout = execFileSync(binary, args, {
    cwd: repoRoot,
    env,
    encoding: "utf8",
    maxBuffer: 16 * 1024 * 1024,
  });
  return JSON.parse(stdout);
}

function summarizeOutput(output) {
  const result = output.result ?? output;
  const snapshot = result.output?.snapshot ?? result.output ?? {};
  const blocks = Array.isArray(snapshot.blocks) ? snapshot.blocks : [];
  return {
    status: result.status ?? output.status ?? "unknown",
    sourceType: snapshot.source?.sourceType,
    blockCount: blocks.length,
    blocks: blocks.map((block) => ({
      ref: block.ref,
      kind: block.kind,
      text: block.text,
    })),
    texts: blocks.map((block) => block.text).filter(Boolean),
    qualityLabel:
      result.diagnostics?.qualityLabel ?? output.diagnostics?.qualityLabel,
    waitStrategy:
      result.diagnostics?.waitStrategy ?? output.diagnostics?.waitStrategy,
  };
}

function resolveAction(action, previousOutput) {
  const refTextIndex = action.indexOf("--ref-text");
  if (refTextIndex === -1) {
    return action;
  }
  const text = action[refTextIndex + 1];
  const refKindIndex = action.indexOf("--ref-kind");
  const kind = refKindIndex === -1 ? undefined : action[refKindIndex + 1];
  const match = previousOutput.blocks?.find(
    (block) =>
      String(block.text ?? "").includes(text) &&
      (kind === undefined || block.kind === kind),
  );
  if (!match?.ref) {
    throw new Error(`Could not resolve ref for text: ${text}`);
  }
  const resolved = [
    ...action.slice(0, refTextIndex),
    "--ref",
    match.ref,
    ...action.slice(refTextIndex + 2),
  ];
  if (refKindIndex === -1) {
    return resolved;
  }
  const nextRefKindIndex = resolved.indexOf("--ref-kind");
  return [
    ...resolved.slice(0, nextRefKindIndex),
    ...resolved.slice(nextRefKindIndex + 2),
  ];
}

function summarizeParity(testCase, playwright, cdpRust) {
  const playwrightText = playwright.texts?.join(" ") ?? "";
  const cdpText = cdpRust.texts?.join(" ") ?? "";
  const missing = testCase.mustContain.filter(
    (text) => !cdpText.includes(text),
  );
  const playwrightRequired = !testCase.cdpOnly;
  return {
    passed:
      (!playwrightRequired || playwright.status !== "failed") &&
      cdpRust.status !== "failed" &&
      missing.length === 0 &&
      (!playwrightRequired || playwright.sourceType === "playwright") &&
      cdpRust.sourceType === "cdp-rust",
    missingFromCdpRust: missing,
    playwrightRequired,
    playwrightContainsExpected: testCase.mustContain.every((text) =>
      playwrightText.includes(text),
    ),
    playwrightSourceType: playwright.sourceType,
    cdpRustSourceType: cdpRust.sourceType,
  };
}
