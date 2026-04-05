export function evidenceSupportedClaims(report) {
  return Array.isArray(report?.evidenceSupportedClaims)
    ? report.evidenceSupportedClaims
    : [];
}

export function contradictedClaims(report) {
  return Array.isArray(report?.contradictedClaims)
    ? report.contradictedClaims
    : [];
}

export function insufficientEvidenceClaims(report) {
  return Array.isArray(report?.insufficientEvidenceClaims)
    ? report.insufficientEvidenceClaims
    : [];
}

export function needsMoreBrowsingClaims(report) {
  return Array.isArray(report?.needsMoreBrowsingClaims)
    ? report.needsMoreBrowsingClaims
    : [];
}

export function claimOutcomes(report) {
  if (Array.isArray(report?.claimOutcomes) && report.claimOutcomes.length > 0) {
    return report.claimOutcomes;
  }

  return [
    ...evidenceSupportedClaims(report).map((claim) => ({
      claimId: claim.claimId,
      statement: claim.statement,
      verdict: "evidence-supported",
      support: claim.support ?? [],
      supportScore: claim.supportScore ?? claim.confidence ?? null,
      citation: claim.citation ?? null,
      verificationVerdict: null,
    })),
    ...contradictedClaims(report).map((claim) => ({
      claimId: claim.claimId,
      statement: claim.statement,
      verdict: "contradicted",
      reason: claim.reason ?? null,
      checkedBlockRefs: claim.checkedBlockRefs ?? [],
      nextActionHint: claim.nextActionHint ?? null,
      verificationVerdict: null,
    })),
    ...insufficientEvidenceClaims(report).map((claim) => ({
      claimId: claim.claimId,
      statement: claim.statement,
      verdict: "insufficient-evidence",
      reason: claim.reason ?? null,
      checkedBlockRefs: claim.checkedBlockRefs ?? [],
      nextActionHint: claim.nextActionHint ?? null,
      verificationVerdict: null,
    })),
    ...needsMoreBrowsingClaims(report).map((claim) => ({
      claimId: claim.claimId,
      statement: claim.statement,
      verdict: "needs-more-browsing",
      reason: claim.reason ?? null,
      checkedBlockRefs: claim.checkedBlockRefs ?? [],
      nextActionHint: claim.nextActionHint ?? null,
      verificationVerdict: null,
    })),
  ];
}

export function claimOutcomeForStatement(report, statement) {
  return (
    claimOutcomes(report).find((claim) => claim.statement === statement) ?? null
  );
}
