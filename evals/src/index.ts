import { readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

type ReleaseReadinessReport = {
  readonly readinessScore: number;
  readonly status: string;
};

type CustomerProxyReport = {
  readonly coreTaskCount: number;
  readonly extendedProxySuccessRate: number;
  readonly status: string;
};

type WorkflowReport = {
  readonly status: string;
  readonly taskProof?: {
    readonly supportedClaimRate?: number;
  };
};

type RealUserResearchReport = {
  readonly status: string;
  readonly averageSupportedClaimRate?: number;
};

export type EvalHarnessStatus = {
  readonly status: "active" | "partial" | "missing-artifacts";
  readonly package: "evals";
  readonly readiness: {
    readonly status: string | null;
    readonly readinessScore: number | null;
  };
  readonly proxy: {
    readonly status: string | null;
    readonly coreTaskCount: number;
    readonly extendedProxySuccessRate: number;
  };
  readonly workflows: {
    readonly reference: string | null;
    readonly staged: string | null;
    readonly publicReference: string | null;
    readonly realUserResearch: string | null;
  };
  readonly coverage: {
    readonly runtimeReady: boolean;
    readonly mixedSourceReady: boolean;
    readonly publicProofReady: boolean;
    readonly realUserResearchReady: boolean;
  };
};

const currentDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(currentDir, "../..");

export function evalHarnessStatus(): EvalHarnessStatus {
  const release = tryReadJson<ReleaseReadinessReport>(
    "fixtures/scenarios/release-readiness/report.json",
  );
  const proxy = tryReadJson<CustomerProxyReport>(
    "fixtures/scenarios/customer-proxy-tasks/report.json",
  );
  const reference = tryReadJson<WorkflowReport>(
    "fixtures/scenarios/reference-research-workflow/report.json",
  );
  const staged = tryReadJson<WorkflowReport>(
    "fixtures/scenarios/staged-reference-workflow/report.json",
  );
  const publicReference = tryReadJson<WorkflowReport>(
    "fixtures/scenarios/public-reference-workflow/report.json",
  );
  const realUserResearch = tryReadJson<RealUserResearchReport>(
    "fixtures/scenarios/real-user-research-benchmark/report.json",
  );

  const runtimeReady =
    release !== null &&
    ["pilot-ready", "alpha-ready"].includes(release.status) &&
    proxy !== null &&
    ["proxy-validated", "core-proxy-validated"].includes(proxy.status) &&
    reference?.status === "ok";
  const mixedSourceReady =
    staged?.status === "ok" && (staged.taskProof?.supportedClaimRate ?? 0) >= 1;
  const publicProofReady = publicReference?.status === "ok";
  const realUserResearchReady =
    realUserResearch?.status === "real-user-validated" &&
    (realUserResearch?.averageSupportedClaimRate ?? 0) >= 1;

  const artifactCount = [
    release,
    proxy,
    reference,
    staged,
    publicReference,
    realUserResearch,
  ].filter(Boolean).length;

  return {
    status: evalHarnessPackageStatus({
      runtimeReady,
      mixedSourceReady,
      publicProofReady,
      realUserResearchReady,
      artifactCount,
    }),
    package: "evals",
    readiness: {
      status: release?.status ?? null,
      readinessScore: release?.readinessScore ?? null,
    },
    proxy: {
      status: proxy?.status ?? null,
      coreTaskCount: proxy?.coreTaskCount ?? 0,
      extendedProxySuccessRate: proxy?.extendedProxySuccessRate ?? 0,
    },
    workflows: {
      reference: reference?.status ?? null,
      staged: staged?.status ?? null,
      publicReference: publicReference?.status ?? null,
      realUserResearch: realUserResearch?.status ?? null,
    },
    coverage: {
      runtimeReady,
      mixedSourceReady,
      publicProofReady,
      realUserResearchReady,
    },
  };
}

function evalHarnessPackageStatus({
  runtimeReady,
  mixedSourceReady,
  publicProofReady,
  realUserResearchReady,
  artifactCount,
}: {
  readonly runtimeReady: boolean;
  readonly mixedSourceReady: boolean;
  readonly publicProofReady: boolean;
  readonly realUserResearchReady: boolean;
  readonly artifactCount: number;
}): EvalHarnessStatus["status"] {
  if (
    runtimeReady &&
    mixedSourceReady &&
    publicProofReady &&
    realUserResearchReady
  ) {
    return "active";
  }
  if (artifactCount > 0) {
    return "partial";
  }
  return "missing-artifacts";
}

function tryReadJson<T>(relativePath: string): T | null {
  try {
    return JSON.parse(
      readFileSync(path.join(repoRoot, relativePath), "utf8"),
    ) as T;
  } catch {
    return null;
  }
}

if (import.meta.url === `file://${process.argv[1]}`) {
  console.log(JSON.stringify(evalHarnessStatus(), null, 2));
}
