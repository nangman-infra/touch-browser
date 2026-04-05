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
  const contradictedClaims = new Map(
    (request.evidenceReport?.contradictedClaims ?? []).map((claim) => [
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
  const needsMoreBrowsingClaims = new Map(
    (request.evidenceReport?.needsMoreBrowsingClaims ?? []).map((claim) => [
      claim.claimId,
      claim,
    ]),
  );
  const blocksById = new Map(snapshotBlocks.map((block) => [block.id, block]));
  const outcomes = claims.map((claim) =>
    verifyClaim(
      claim,
      supportedClaims,
      contradictedClaims,
      unsupportedClaims,
      needsMoreBrowsingClaims,
      blocksById,
    ),
  );

  process.stdout.write(`${JSON.stringify({ outcomes }, null, 2)}\n`);
}

function verifyClaim(
  claim,
  supportedClaims,
  contradictedClaims,
  unsupportedClaims,
  needsMoreBrowsingClaims,
  blocksById,
) {
  const supported = supportedClaims.get(claim.id);
  const contradicted = contradictedClaims.get(claim.id);
  const unsupported = unsupportedClaims.get(claim.id);
  const needsMoreBrowsing = needsMoreBrowsingClaims.get(claim.id);

  if (contradicted) {
    return {
      claimId: claim.id,
      statement: claim.statement,
      verdict: "contradicted",
      verifierScore: 0.1,
      notes: `Base extractor returned ${contradicted.reason}.`,
    };
  }

  if (needsMoreBrowsing) {
    return {
      claimId: claim.id,
      statement: claim.statement,
      verdict: "needs-more-browsing",
      verifierScore: 0.28,
      notes:
        needsMoreBrowsing.nextActionHint ??
        `Base extractor returned ${needsMoreBrowsing.reason}.`,
    };
  }

  if (unsupported) {
    return {
      claimId: claim.id,
      statement: claim.statement,
      verdict: "insufficient-evidence",
      verifierScore: 0.18,
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
  const numericCheck = compareNumericExpressions(
    numericExpressions(claim.statement),
    numericExpressions(combinedSupportText),
  );
  const minimumAnchorCoverage = requiredAnchorCoverage(anchorTokens.length);
  const verified =
    anchorCoverage >= minimumAnchorCoverage &&
    qualifierCoverage >= 1 &&
    numericCheck.kind === "match" &&
    Number(supported.supportScore ?? 0) >= 0.6;
  const verdict =
    numericCheck.kind === "mismatch"
      ? "contradicted"
      : verified
        ? "verified"
        : numericCheck.kind === "missing-support-number" ||
            qualifierCoverage < 1 ||
            anchorCoverage < minimumAnchorCoverage
          ? "needs-more-browsing"
          : "insufficient-evidence";

  return {
    claimId: claim.id,
    statement: claim.statement,
    verdict,
    verifierScore: Number(
      Math.max(
        0,
        Math.min(
          1,
          verdict === "contradicted"
            ? 0.08
            : verified
              ? 0.55 +
                Number(supported.supportScore ?? 0) * 0.25 +
                anchorCoverage * 0.2
              : verdict === "needs-more-browsing"
                ? 0.24 + anchorCoverage * 0.2 + qualifierCoverage * 0.1
                : 0.16 + anchorCoverage * 0.2 + qualifierCoverage * 0.1,
        ),
      ).toFixed(2),
    ),
    notes: [
      `anchorCoverage=${anchorCoverage.toFixed(2)}`,
      `qualifierCoverage=${qualifierCoverage.toFixed(2)}`,
      `numericCheck=${numericCheck.kind}`,
      `supportScore=${Number(supported.supportScore ?? 0).toFixed(2)}`,
      verified
        ? "Conservative verifier accepted the evidence set."
        : verdict === "contradicted"
          ? "Conservative verifier found a conflicting numeric or qualifier detail."
          : verdict === "needs-more-browsing"
            ? "Conservative verifier requires a more specific source page."
            : "Conservative verifier left the claim insufficiently grounded.",
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

function numericExpressions(text) {
  const tokens = normalizeText(text).split(" ").filter(Boolean);
  const expressions = [];
  for (let index = 0; index < tokens.length; index += 1) {
    const token = tokens[index];
    if (/^\d+$/.test(token)) {
      const expression = {
        value: token,
        unit: normalizeUnit(tokens[index + 1] ?? ""),
      };
      if (
        !expressions.some(
          (existing) =>
            existing.value === expression.value &&
            existing.unit === expression.unit,
        )
      ) {
        expressions.push(expression);
      }
    }
  }
  return expressions;
}

function normalizeUnit(token) {
  switch (token) {
    case "second":
    case "seconds":
    case "sec":
    case "secs":
      return "second";
    case "minute":
    case "minutes":
    case "min":
    case "mins":
      return "minute";
    case "hour":
    case "hours":
    case "hr":
    case "hrs":
      return "hour";
    case "day":
    case "days":
      return "day";
    default:
      return null;
  }
}

function compareNumericExpressions(claimNumbers, supportNumbers) {
  if (claimNumbers.length === 0) {
    return { kind: "match" };
  }
  if (supportNumbers.length === 0) {
    return { kind: "missing-support-number" };
  }
  const matched = claimNumbers.every((claimNumber) =>
    supportNumbers.some(
      (supportNumber) =>
        supportNumber.value === claimNumber.value &&
        (!claimNumber.unit ||
          !supportNumber.unit ||
          supportNumber.unit === claimNumber.unit),
    ),
  );
  return { kind: matched ? "match" : "mismatch" };
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
