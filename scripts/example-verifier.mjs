#!/usr/bin/env node

async function main() {
  const request = JSON.parse(await readStdin());
  const claims = Array.isArray(request.claims) ? request.claims : [];
  const snapshotBlocks = Array.isArray(request.snapshot?.blocks)
    ? request.snapshot.blocks
    : [];
  const evidenceIndex = indexEvidenceClaims(request.evidenceReport);
  const blocksById = new Map(snapshotBlocks.map((block) => [block.id, block]));
  const outcomes = claims.map((claim) =>
    verifyClaim(claim, evidenceIndex, blocksById),
  );

  process.stdout.write(`${JSON.stringify({ outcomes }, null, 2)}\n`);
}

function indexEvidenceClaims(evidenceReport) {
  return {
    supportedClaims: claimsById(evidenceReport?.evidenceSupportedClaims ?? []),
    contradictedClaims: claimsById(evidenceReport?.contradictedClaims ?? []),
    unsupportedClaims: claimsById(
      evidenceReport?.insufficientEvidenceClaims ?? [],
    ),
    needsMoreBrowsingClaims: claimsById(
      evidenceReport?.needsMoreBrowsingClaims ?? [],
    ),
  };
}

function claimsById(claims) {
  return new Map(claims.map((claim) => [claim.claimId, claim]));
}

function verifyClaim(claim, evidenceIndex, blocksById) {
  const directOutcome = directOutcomeForClaim(claim, evidenceIndex);
  if (directOutcome) {
    return directOutcome;
  }

  const supported = evidenceIndex.supportedClaims.get(claim.id);
  const analysis = analyzeSupportedClaim(claim, supported, blocksById);

  return {
    claimId: claim.id,
    statement: claim.statement,
    verdict: analysis.verdict,
    verifierScore: verifierScoreForAnalysis(analysis, supported),
    notes: notesForAnalysis(analysis, supported),
  };
}

function directOutcomeForClaim(claim, evidenceIndex) {
  const contradicted = evidenceIndex.contradictedClaims.get(claim.id);
  if (contradicted) {
    return buildOutcome(
      claim,
      "contradicted",
      0.1,
      `Base extractor returned ${contradicted.reason}.`,
    );
  }

  const needsMoreBrowsing = evidenceIndex.needsMoreBrowsingClaims.get(claim.id);
  if (needsMoreBrowsing) {
    return buildOutcome(
      claim,
      "needs-more-browsing",
      0.28,
      needsMoreBrowsing.nextActionHint ??
        `Base extractor returned ${needsMoreBrowsing.reason}.`,
    );
  }

  const unsupported = evidenceIndex.unsupportedClaims.get(claim.id);
  if (unsupported) {
    return buildOutcome(
      claim,
      "insufficient-evidence",
      0.18,
      `Base extractor returned ${unsupported.reason}.`,
    );
  }

  if (!evidenceIndex.supportedClaims.has(claim.id)) {
    return buildOutcome(
      claim,
      "unresolved",
      0.15,
      "No evidence-supported claim was returned by the base extractor.",
    );
  }

  return null;
}

function buildOutcome(claim, verdict, verifierScore, notes) {
  return {
    claimId: claim.id,
    statement: claim.statement,
    verdict,
    verifierScore,
    notes,
  };
}

function analyzeSupportedClaim(claim, supported, blocksById) {
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

  return {
    anchorCoverage,
    qualifierCoverage,
    numericCheck,
    minimumAnchorCoverage,
    verified,
    verdict: supportedVerdict({
      anchorCoverage,
      qualifierCoverage,
      numericCheck,
      minimumAnchorCoverage,
      verified,
    }),
  };
}

function supportedVerdict({
  anchorCoverage,
  qualifierCoverage,
  numericCheck,
  minimumAnchorCoverage,
  verified,
}) {
  if (numericCheck.kind === "mismatch") {
    return "contradicted";
  }
  if (verified) {
    return "verified";
  }
  if (
    numericCheck.kind === "missing-support-number" ||
    qualifierCoverage < 1 ||
    anchorCoverage < minimumAnchorCoverage
  ) {
    return "needs-more-browsing";
  }
  return "insufficient-evidence";
}

function verifierScoreForAnalysis(analysis, supported) {
  const supportScore = Number(supported.supportScore ?? 0);
  let rawScore =
    0.16 + analysis.anchorCoverage * 0.2 + analysis.qualifierCoverage * 0.1;

  if (analysis.verdict === "contradicted") {
    rawScore = 0.08;
  } else if (analysis.verified) {
    rawScore = 0.55 + supportScore * 0.25 + analysis.anchorCoverage * 0.2;
  } else if (analysis.verdict === "needs-more-browsing") {
    rawScore =
      0.24 + analysis.anchorCoverage * 0.2 + analysis.qualifierCoverage * 0.1;
  }

  return Number(Math.max(0, Math.min(1, rawScore)).toFixed(2));
}

function notesForAnalysis(analysis, supported) {
  return [
    `anchorCoverage=${analysis.anchorCoverage.toFixed(2)}`,
    `qualifierCoverage=${analysis.qualifierCoverage.toFixed(2)}`,
    `numericCheck=${analysis.numericCheck.kind}`,
    `supportScore=${Number(supported.supportScore ?? 0).toFixed(2)}`,
    verificationNote(analysis),
  ].join("; ");
}

function verificationNote(analysis) {
  if (analysis.verified) {
    return "Conservative verifier accepted the evidence set.";
  }
  if (analysis.verdict === "contradicted") {
    return "Conservative verifier found a conflicting numeric or qualifier detail.";
  }
  if (analysis.verdict === "needs-more-browsing") {
    return "Conservative verifier requires a more specific source page.";
  }
  return "Conservative verifier left the claim insufficiently grounded.";
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
    .replaceAll(/[^a-z0-9]+/g, " ")
    .trim()
    .replaceAll(/\s+/g, " ");
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

try {
  await main();
} catch (error) {
  process.stderr.write(`${String(error?.stack ?? error)}\n`);
  process.exitCode = 1;
}
