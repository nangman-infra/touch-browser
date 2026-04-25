import { spawn } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";

type PolicyDecision = "allow" | "review" | "block";
type ClaimVerdict =
  | "evidence-supported"
  | "contradicted"
  | "insufficient-evidence"
  | "needs-more-browsing";

type QaSiteManifest = {
  version: string;
  sites: QaSiteScenario[];
};

type QaSiteScenario = {
  id: string;
  label: string;
  target: string;
  mode: "stateless-extract" | "session-follow";
  verifierCommand?: string;
  claims: Array<{
    statement: string;
    expectedVerdict: ClaimVerdict;
  }>;
  readView?: {
    mustContain: string[];
    mustNotContain: string[];
  };
  sessionFlow?: {
    followRef: string;
    afterReadMustContain: string[];
  };
  expected: {
    supported: boolean;
    contradicted: boolean;
    needsMoreBrowsing: boolean;
    policy: {
      pageDecision: PolicyDecision;
      actionDecision: PolicyDecision;
    };
  };
};

type ExtractResult = {
  extract: {
    output: {
      claimOutcomes: Array<{
        statement: string;
        verdict: ClaimVerdict;
        verdictExplanation?: string;
      }>;
      evidenceSupportedClaims?: Array<{ statement: string }>;
      contradictedClaims?: Array<{ statement: string }>;
      needsMoreBrowsingClaims?: Array<{
        statement: string;
        nextActionHint?: string;
      }>;
    };
    policy: {
      pageRisk: { decision: PolicyDecision };
      actionRisk: { decision: PolicyDecision };
    };
  };
};

type OpenResult = {
  policy: {
    pageRisk: { decision: PolicyDecision };
    actionRisk: { decision: PolicyDecision };
  };
};

const manifest = JSON.parse(
  readFileSync(
    join(
      repoRoot,
      "fixtures",
      "scenarios",
      "qa-site-regression",
      "target-sites.json",
    ),
    "utf8",
  ),
) as QaSiteManifest;

const publicWebQaEnabled =
  process.env.TOUCH_BROWSER_RUN_PUBLIC_WEB_QA?.trim() === "1";

describe("qa site regression cli e2e", () => {
  for (const site of manifest.sites.filter(
    (candidate) => candidate.mode === "session-follow",
  )) {
    it(`keeps fixture session flow stable for ${site.id}`, async () => {
      const tempDir = mkdtempSync(join(tmpdir(), "touch-browser-qa-"));
      const sessionFile = join(tempDir, "session.json");

      try {
        const open = await runTouchBrowserJson<OpenResult>([
          "open",
          site.target,
          "--session-file",
          sessionFile,
        ]);
        expect(open.policy.pageRisk.decision).toBe(
          site.expected.policy.pageDecision,
        );
        expect(open.policy.actionRisk.decision).toBe(
          site.expected.policy.actionDecision,
        );

        const followRef = site.sessionFlow?.followRef;
        if (!followRef) {
          throw new Error(`missing sessionFlow.followRef for ${site.id}`);
        }
        await runTouchBrowserJson([
          "follow",
          "--session-file",
          sessionFile,
          "--ref",
          followRef,
        ]);

        const sessionRead = await runTouchBrowserText([
          "session-read",
          "--session-file",
          sessionFile,
          "--main-only",
        ]);
        for (const fragment of site.sessionFlow?.afterReadMustContain ?? []) {
          expect(sessionRead).toContain(fragment);
        }

        const extract = await runTouchBrowserJson<ExtractResult>([
          "session-extract",
          "--session-file",
          sessionFile,
          ...site.claims.flatMap((claim) => ["--claim", claim.statement]),
        ]);

        assertScenarioExtract(site, extract);
      } finally {
        rmSync(tempDir, { force: true, recursive: true });
      }
    }, 60_000);
  }

  const publicWebIt = publicWebQaEnabled ? it : it.skip;
  for (const site of manifest.sites.filter(
    (candidate) => candidate.mode === "stateless-extract",
  )) {
    publicWebIt(
      `keeps public web regression stable for ${site.id}`,
      async () => {
        const readView = await runTouchBrowserText([
          "read-view",
          site.target,
          "--main-only",
        ]);

        for (const fragment of site.readView?.mustContain ?? []) {
          expect(readView).toContain(fragment);
        }
        for (const fragment of site.readView?.mustNotContain ?? []) {
          expect(readView).not.toContain(fragment);
        }

        const extract = await runTouchBrowserJson<ExtractResult>([
          "extract",
          site.target,
          ...site.claims.flatMap((claim) => ["--claim", claim.statement]),
          ...(site.verifierCommand
            ? ["--verifier-command", site.verifierCommand]
            : []),
        ]);

        assertScenarioExtract(site, extract);

        if (site.id === "chrome-developers-blog") {
          const chromeOutcome = extract.extract.output.claimOutcomes.find(
            (outcome) =>
              outcome.statement ===
              "This page is the official reference page for Chrome developers.",
          );
          expect(chromeOutcome?.verdictExplanation).toContain(
            "more specific source page",
          );
        }
      },
      90_000,
    );
  }
});

function assertScenarioExtract(site: QaSiteScenario, extract: ExtractResult) {
  expect(extract.extract.policy.pageRisk.decision).toBe(
    site.expected.policy.pageDecision,
  );
  expect(extract.extract.policy.actionRisk.decision).toBe(
    site.expected.policy.actionDecision,
  );

  const claimOutcomes = new Map(
    extract.extract.output.claimOutcomes.map((outcome) => [
      outcome.statement,
      outcome.verdict,
    ]),
  );

  for (const claim of site.claims) {
    expect(claimOutcomes.get(claim.statement)).toBe(claim.expectedVerdict);
  }

  expect(
    (extract.extract.output.evidenceSupportedClaims ?? []).length > 0,
  ).toBe(site.expected.supported);
  expect((extract.extract.output.contradictedClaims ?? []).length > 0).toBe(
    site.expected.contradicted,
  );
  expect(
    (extract.extract.output.needsMoreBrowsingClaims ?? []).length > 0,
  ).toBe(site.expected.needsMoreBrowsing);
}

async function runTouchBrowserJson<T>(args: string[]): Promise<T> {
  return JSON.parse(await runTouchBrowserText(args)) as T;
}

async function runTouchBrowserText(args: string[]): Promise<string> {
  return (
    await runCommand(
      "cargo",
      ["run", "-q", "-p", "touch-browser-cli", "--", ...args],
      {
        cwd: repoRoot,
        env: {
          ...process.env,
          TOUCH_BROWSER_REPO_ROOT: repoRoot,
        },
      },
    )
  ).trim();
}

function runCommand(
  command: string,
  args: string[],
  options: {
    cwd: string;
    env?: NodeJS.ProcessEnv;
  },
): Promise<string> {
  return new Promise<string>((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    });

    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];

    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(Buffer.concat(stderr).toString("utf8").trim()));
        return;
      }
      resolve(Buffer.concat(stdout).toString("utf8"));
    });
  });
}
