import { spawn } from "node:child_process";

import { describe, expect, it } from "vitest";

import { repoRoot } from "../support/paths.js";

type VerifierOutcome = {
  claimId: string;
  verdict: string;
  notes: string;
};

type VerifierResponse = {
  outcomes: VerifierOutcome[];
};

describe("example verifier", () => {
  it("marks qualifier and anchor mismatches as needs-more-browsing", async () => {
    const payload = {
      claims: [
        {
          id: "c1",
          statement: "ECS supports GPU instances natively.",
        },
      ],
      snapshot: {
        blocks: [
          {
            id: "b1",
            text: "Amazon ECS is a fully managed container service.",
          },
          {
            id: "b2",
            text: "Managed instances support GPU acceleration for selected workloads.",
          },
        ],
      },
      evidenceReport: {
        evidenceSupportedClaims: [
          {
            claimId: "c1",
            statement: "ECS supports GPU instances natively.",
            supportScore: 0.81,
            support: ["b1", "b2"],
          },
        ],
        insufficientEvidenceClaims: [],
      },
    };

    const output = await runVerifier(payload);

    expect(output.outcomes).toHaveLength(1);
    const [outcome] = output.outcomes;
    expect(outcome).toBeDefined();
    if (!outcome) {
      throw new Error("expected one verifier outcome");
    }
    expect(outcome.claimId).toBe("c1");
    expect(outcome.verdict).toBe("needs-more-browsing");
    expect(outcome.notes).toContain("qualifierCoverage=0.00");
  });

  it("keeps review-recommended supported claims unresolved", async () => {
    const payload = {
      claims: [
        {
          id: "c2",
          statement: "Kubernetes is written in Python.",
        },
      ],
      snapshot: {
        blocks: [
          {
            id: "b7",
            text: "Kubernetes is an open source container orchestration platform.",
          },
        ],
      },
      evidenceReport: {
        evidenceSupportedClaims: [
          {
            claimId: "c2",
            statement: "Kubernetes is written in Python.",
            supportScore: 0.79,
            support: ["b7"],
          },
        ],
        claimOutcomes: [
          {
            claimId: "c2",
            statement: "Kubernetes is written in Python.",
            verdict: "evidence-supported",
            support: ["b7"],
            supportScore: 0.79,
            confidenceBand: "review",
            reviewRecommended: true,
          },
        ],
      },
    };

    const output = await runVerifier(payload);

    expect(output.outcomes).toHaveLength(1);
    const [outcome] = output.outcomes;
    expect(outcome).toBeDefined();
    if (!outcome) {
      throw new Error("expected one verifier outcome");
    }
    expect(outcome.claimId).toBe("c2");
    expect(outcome.verdict).toBe("needs-more-browsing");
    expect(outcome.notes).toContain("reviewRecommended=true");
    expect(outcome.notes).toContain("confidenceBand=review");
  });
});

function runVerifier(payload: unknown) {
  return new Promise<VerifierResponse>((resolve, reject) => {
    const child = spawn("node", ["scripts/example-verifier.mjs"], {
      cwd: repoRoot,
      stdio: ["pipe", "pipe", "pipe"],
    });

    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];

    child.stdout.on("data", (chunk) => stdout.push(chunk));
    child.stderr.on("data", (chunk) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      if (code !== 0) {
        reject(new Error(Buffer.concat(stderr).toString("utf8")));
        return;
      }

      resolve(
        JSON.parse(Buffer.concat(stdout).toString("utf8")) as VerifierResponse,
      );
    });

    child.stdin.write(JSON.stringify(payload));
    child.stdin.end();
  });
}
