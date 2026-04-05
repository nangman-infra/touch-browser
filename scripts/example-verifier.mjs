#!/usr/bin/env node

main().catch((error) => {
  process.stderr.write(`${String(error?.stack ?? error)}\n`);
  process.exitCode = 1;
});

async function main() {
  const request = JSON.parse(await readStdin());
  const claims = Array.isArray(request.claims) ? request.claims : [];
  const snapshotBlocks = Array.isArray(request.snapshot?.blocks)
    ? request.snapshot.blocks
    : [];
  const supportedClaims = new Map(
    (request.evidenceReport?.evidenceSupportedClaims ?? []).map((claim) => [
      claim.claimId,
      claim,
    ]),
  );
  const unsupportedClaims = new Map(
    (request.evidenceReport?.insufficientEvidenceClaims ?? []).map((claim) => [
      claim.claimId,
      claim,
    ]),
  );
  const blocksById = new Map(snapshotBlocks.map((block) => [block.id, block]));
  const outcomes = claims.map((claim) =>
    verifyClaim(claim, supportedClaims, unsupportedClaims, blocksById),
  );

  process.stdout.write(`${JSON.stringify({ outcomes }, null, 2)}\n`);
}

function verifyClaim(claim, supportedClaims, unsupportedClaims, blocksById) {
  const supported = supportedClaims.get(claim.id);
  const unsupported = unsupportedClaims.get(claim.id);

  if (unsupported) {
    return {
      claimId: claim.id,
      statement: claim.statement,
      verdict:
        unsupported.reason === "contradictory-evidence"
          ? "contradicted"
          : "unresolved",
      verifierScore:
        unsupported.reason === "contradictory-evidence" ? 0.12 : 0.2,
      notes: `Base extractor returned ${unsupported.reason}.`,
    };
  }

  if (!supported) {
    return {
      claimId: claim.id,
      statement: claim.statement,
      verdict: "unresolved",
      verifierScore: 0.15,
      notes: "No evidence-supported claim was returned by the base extractor.",
    };
  }

  const supportTexts = (supported.support ?? [])
    .map((id) => blocksById.get(id)?.text ?? "")
    .filter(Boolean);
  const combinedSupportText = supportTexts.join(" ");
  const anchorTokens = getAnchorTokens(claim.statement);
  const qualifierTokens = getQualifierTokens(claim.statement);
  const supportTokens = tokenizeSignificant(combinedSupportText);
  const supportAllTokens = tokenizeAll(combinedSupportText);
  const anchorCoverage = anchorTokens.length
    ? coverageRatio(anchorTokens, supportTokens)
    : 1;
  const qualifierCoverage = qualifierTokens.length
    ? coverageRatio(qualifierTokens, supportAllTokens)
    : 1;
  const minimumAnchorCoverage = requiredAnchorCoverage(anchorTokens.length);
  const verified =
    anchorCoverage >= minimumAnchorCoverage &&
    qualifierCoverage >= 1 &&
    Number(supported.supportScore ?? 0) >= 0.6;

  return {
    claimId: claim.id,
    statement: claim.statement,
    verdict: verified ? "verified" : "unresolved",
    verifierScore: Number(
      Math.max(
        0,
        Math.min(
          1,
          verified
            ? 0.55 +
                Number(supported.supportScore ?? 0) * 0.25 +
                anchorCoverage * 0.2
            : 0.2 + anchorCoverage * 0.25 + qualifierCoverage * 0.1,
        ),
      ).toFixed(2),
    ),
    notes: [
      `anchorCoverage=${anchorCoverage.toFixed(2)}`,
      `qualifierCoverage=${qualifierCoverage.toFixed(2)}`,
      `supportScore=${Number(supported.supportScore ?? 0).toFixed(2)}`,
      verified
        ? "Conservative verifier accepted the evidence set."
        : "Conservative verifier left the claim unresolved.",
    ].join("; "),
  };
}

function readStdin() {
  return new Promise((resolve, reject) => {
    const chunks = [];
    process.stdin.on("data", (chunk) => chunks.push(chunk));
    process.stdin.on("end", () =>
      resolve(Buffer.concat(chunks).toString("utf8").trim()),
    );
    process.stdin.on("error", reject);
  });
}

function normalizeText(text) {
  return String(text)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim()
    .replace(/\s+/g, " ");
}

function stemToken(token) {
  let stemmed = token;
  for (const suffix of ["ing", "ed", "ly", "es", "s"]) {
    if (stemmed.length > suffix.length + 2 && stemmed.endsWith(suffix)) {
      stemmed = stemmed.slice(0, -suffix.length);
      break;
    }
  }
  return stemmed;
}

function tokenizeAll(text) {
  return [
    ...new Set(normalizeText(text).split(" ").filter(Boolean).map(stemToken)),
  ];
}

function tokenizeSignificant(text) {
  return tokenizeAll(text).filter(
    (token) =>
      /^\d+$/.test(token) || (token.length >= 3 && !STOP_WORDS.has(token)),
  );
}

function getAnchorTokens(text) {
  return tokenizeSignificant(text).filter(
    (token) =>
      token.length >= 5 &&
      !ANCHOR_STOP_WORDS.has(token) &&
      !QUALIFIER_TOKENS.has(token),
  );
}

function getQualifierTokens(text) {
  return tokenizeAll(text).filter((token) => QUALIFIER_TOKENS.has(token));
}

function coverageRatio(claimTokens, supportTokens) {
  if (claimTokens.length === 0) {
    return 1;
  }
  const matched = claimTokens.filter((claimToken) =>
    supportTokens.some((supportToken) => tokensMatch(claimToken, supportToken)),
  ).length;
  return matched / claimTokens.length;
}

function requiredAnchorCoverage(anchorCount) {
  if (anchorCount === 0) return 0;
  if (anchorCount <= 2) return 1;
  if (anchorCount === 3) return 0.67;
  return 0.6;
}

function tokensMatch(left, right) {
  return (
    left === right ||
    (left.length >= 4 && right.startsWith(left)) ||
    (right.length >= 4 && left.startsWith(right))
  );
}

const STOP_WORDS = new Set([
  "the",
  "and",
  "for",
  "with",
  "that",
  "this",
  "from",
  "into",
  "your",
  "must",
  "now",
  "are",
  "all",
  "per",
  "there",
  "page",
  "include",
  "includes",
  "includ",
  "contain",
  "contains",
  "list",
  "built",
  "flow",
  "runtime",
  "plan",
  "touch",
  "browser",
]);

const ANCHOR_STOP_WORDS = new Set([
  "support",
  "avail",
  "available",
  "provid",
  "service",
  "system",
  "platform",
]);

const QUALIFIER_TOKENS = new Set([
  "all",
  "every",
  "fully",
  "native",
  "global",
  "worldwide",
  "only",
  "always",
  "never",
  "entire",
]);
