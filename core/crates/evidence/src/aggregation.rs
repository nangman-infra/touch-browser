use std::collections::{BTreeMap, BTreeSet};

use touch_browser_contracts::{
    EvidenceClaimVerdict, EvidenceGuardFailure, EvidenceGuardKind, SnapshotBlock,
    SnapshotBlockKind, UnsupportedClaimReason,
};

use crate::{
    contradiction::contradiction_detected,
    normalization::{
        anchor_tokens, normalize_text, token_overlap_ratio, tokenize_all, tokenize_significant,
        tokens_match, ClaimAnalysisInput,
    },
    scoring::{
        block_search_text, claim_token_weight, exact_match_bonus, is_narrative_aggregate_block,
        nearest_heading_context, numeric_overlap_ratio, primary_heading_context, round_confidence,
        weighted_token_overlap_ratio, ScoredCandidate, ScoringContext,
    },
    ClaimRequest, ClaimResolution,
};

pub(crate) struct GuardAssessment {
    pub(crate) contradiction_reason: Option<UnsupportedClaimReason>,
    pub(crate) guard_failures: Vec<EvidenceGuardFailure>,
    pub(crate) next_action_hint: Option<String>,
}

struct GuardCheck {
    contradiction_reason: Option<UnsupportedClaimReason>,
    failure: Option<EvidenceGuardFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumericExpression {
    value: String,
    unit: Option<String>,
}

impl NumericExpression {
    fn render(&self) -> String {
        match &self.unit {
            Some(unit) => format!("{} {}", self.value, unit),
            None => self.value.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeProfile {
    Unknown,
    Universal,
    Exclusive,
    Limited,
}

impl ScopeProfile {
    fn requires_explicit_support(self) -> bool {
        matches!(self, ScopeProfile::Universal | ScopeProfile::Exclusive)
    }

    fn label(self) -> &'static str {
        match self {
            ScopeProfile::Unknown => "unknown",
            ScopeProfile::Universal => "universal",
            ScopeProfile::Exclusive => "exclusive",
            ScopeProfile::Limited => "limited",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusProfile {
    Unknown,
    Preview,
    GenerallyAvailable,
    Deprecated,
}

impl StatusProfile {
    fn requires_explicit_support(self) -> bool {
        matches!(
            self,
            StatusProfile::Preview | StatusProfile::GenerallyAvailable
        )
    }

    fn label(self) -> &'static str {
        match self {
            StatusProfile::Unknown => "unknown",
            StatusProfile::Preview => "preview",
            StatusProfile::GenerallyAvailable => "general-availability",
            StatusProfile::Deprecated => "deprecated",
        }
    }
}

pub(crate) fn checked_refs(scored: &[ScoredCandidate<'_>]) -> Vec<String> {
    scored
        .iter()
        .take(3)
        .map(|candidate| candidate.block.stable_ref.clone())
        .collect()
}

pub(crate) fn contradictory_support<'a>(
    scored: &[ScoredCandidate<'a>],
) -> Vec<ScoredCandidate<'a>> {
    scored
        .iter()
        .filter(|candidate| candidate.contradictory && candidate.score >= 0.05)
        .take(3)
        .cloned()
        .collect()
}

pub(crate) fn contradiction_resolution<'a>(
    claim: &ClaimRequest,
    contradictory_support: &[ScoredCandidate<'a>],
    blocks: &'a [SnapshotBlock],
    analysis: &ClaimAnalysisInput,
) -> Option<ClaimResolution<'a>> {
    if contradictory_support.is_empty() {
        return None;
    }

    let contradiction_checked_refs = contradictory_support
        .iter()
        .map(|candidate| candidate.block.stable_ref.clone())
        .collect::<Vec<_>>();
    let assessment = assess_support_guards(
        claim,
        contradictory_support,
        blocks,
        &analysis.claim_anchor_tokens,
        &analysis.claim_qualifier_tokens,
    );
    let reason = assessment
        .contradiction_reason
        .clone()
        .or(Some(UnsupportedClaimReason::NegationMismatch))?;
    let mut guard_failures = assessment.guard_failures;
    if !guard_failures
        .iter()
        .any(|failure| matches!(failure.kind, EvidenceGuardKind::Negation))
    {
        guard_failures.push(EvidenceGuardFailure {
            kind: EvidenceGuardKind::Negation,
            detail:
                "The retrieved support contains polarity language that conflicts with the claim."
                    .to_string(),
        });
    }

    Some(ClaimResolution {
        verdict: EvidenceClaimVerdict::Contradicted,
        support: contradictory_support.to_vec(),
        confidence: None,
        reason: Some(reason),
        checked_refs: contradiction_checked_refs,
        guard_failures,
        next_action_hint: None,
    })
}

pub(crate) fn non_contradictory_candidates<'a>(
    scored: Vec<ScoredCandidate<'a>>,
) -> Vec<ScoredCandidate<'a>> {
    scored
        .into_iter()
        .filter(|candidate| !candidate.contradictory)
        .collect()
}

pub(crate) fn no_candidate_resolution<'a>(
    contradictory_exists: bool,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    if contradictory_exists {
        ClaimResolution {
            verdict: EvidenceClaimVerdict::Contradicted,
            support: Vec::new(),
            confidence: None,
            reason: Some(UnsupportedClaimReason::ContradictoryEvidence),
            checked_refs,
            guard_failures: vec![EvidenceGuardFailure {
                kind: EvidenceGuardKind::Negation,
                detail: "Observed support blocks contradict the claim polarity.".to_string(),
            }],
            next_action_hint: None,
        }
    } else {
        ClaimResolution {
            verdict: EvidenceClaimVerdict::InsufficientEvidence,
            support: Vec::new(),
            confidence: None,
            reason: Some(UnsupportedClaimReason::NoSupportingBlock),
            checked_refs,
            guard_failures: Vec::new(),
            next_action_hint: None,
        }
    }
}

pub(crate) fn top_support_candidates<'a>(
    non_contradictory: Vec<ScoredCandidate<'a>>,
) -> Vec<ScoredCandidate<'a>> {
    non_contradictory
        .into_iter()
        .filter(|candidate| candidate.score >= 0.22)
        .take(3)
        .collect()
}

pub(crate) fn no_top_support_resolution<'a>(
    contradictory_exists: bool,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    let reason = if contradictory_exists {
        UnsupportedClaimReason::ContradictoryEvidence
    } else {
        UnsupportedClaimReason::InsufficientConfidence
    };

    ClaimResolution {
        verdict: if contradictory_exists {
            EvidenceClaimVerdict::Contradicted
        } else {
            EvidenceClaimVerdict::InsufficientEvidence
        },
        support: Vec::new(),
        confidence: None,
        reason: Some(reason),
        checked_refs,
        guard_failures: Vec::new(),
        next_action_hint: None,
    }
}

pub(crate) fn effective_support_score(
    claim: &ClaimRequest,
    analysis: &ClaimAnalysisInput,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    scoring_context: &ScoringContext,
    best_score: f64,
) -> f64 {
    let aggregated_score = aggregate_support_score(
        claim,
        &analysis.normalized_claim,
        &analysis.claim_tokens,
        &analysis.claim_numeric_tokens,
        top_support,
        blocks,
        scoring_context,
    );
    best_score.max(aggregated_score)
}

pub(crate) fn guarded_resolution<'a>(
    claim: &ClaimRequest,
    analysis: &ClaimAnalysisInput,
    top_support: &[ScoredCandidate<'a>],
    checked_refs: &[String],
    assessment: &GuardAssessment,
    effective_score: f64,
    support_threshold: f64,
) -> Option<ClaimResolution<'a>> {
    if let Some(reason) = assessment.contradiction_reason.clone() {
        return Some(ClaimResolution {
            verdict: EvidenceClaimVerdict::Contradicted,
            support: top_support.to_vec(),
            confidence: None,
            reason: Some(reason),
            checked_refs: checked_refs.to_vec(),
            guard_failures: assessment.guard_failures.clone(),
            next_action_hint: None,
        });
    }

    if effective_score >= support_threshold && assessment.guard_failures.is_empty() {
        if button_claim_requires_more_browsing(&analysis.claim_tokens, top_support) {
            return Some(ClaimResolution {
                verdict: EvidenceClaimVerdict::NeedsMoreBrowsing,
                support: top_support.to_vec(),
                confidence: None,
                reason: Some(UnsupportedClaimReason::NeedsMoreBrowsing),
                checked_refs: checked_refs.to_vec(),
                guard_failures: Vec::new(),
                next_action_hint: Some(
                    "Browse a more specific source page before answering.".to_string(),
                ),
            });
        }
        return None;
    }

    let verdict = if should_keep_browsing(effective_score, assessment, claim) {
        EvidenceClaimVerdict::NeedsMoreBrowsing
    } else {
        EvidenceClaimVerdict::InsufficientEvidence
    };

    Some(ClaimResolution {
        verdict: verdict.clone(),
        support: top_support.to_vec(),
        confidence: None,
        reason: Some(if verdict == EvidenceClaimVerdict::NeedsMoreBrowsing {
            UnsupportedClaimReason::NeedsMoreBrowsing
        } else {
            UnsupportedClaimReason::InsufficientConfidence
        }),
        checked_refs: checked_refs.to_vec(),
        guard_failures: assessment.guard_failures.clone(),
        next_action_hint: assessment.next_action_hint.clone(),
    })
}

pub(crate) fn supported_resolution<'a>(
    best_score: f64,
    top_support: Vec<ScoredCandidate<'a>>,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    let confidence = round_confidence(
        best_score.max(
            top_support
                .iter()
                .map(|candidate| candidate.score)
                .fold(0.0, f64::max),
        ),
    );

    ClaimResolution {
        verdict: EvidenceClaimVerdict::EvidenceSupported,
        support: top_support,
        confidence: Some(confidence),
        reason: None,
        checked_refs,
        guard_failures: Vec::new(),
        next_action_hint: None,
    }
}

pub(crate) fn support_acceptance_threshold(
    top_support: &[ScoredCandidate<'_>],
    assessment: &GuardAssessment,
) -> f64 {
    if !assessment.guard_failures.is_empty() || top_support.len() < 2 {
        return 0.52;
    }

    let narrative_support_count = top_support
        .iter()
        .filter(|candidate| is_narrative_aggregate_block(candidate.block))
        .count();
    if narrative_support_count >= 2 {
        0.46
    } else {
        0.52
    }
}

fn button_claim_requires_more_browsing(
    claim_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
) -> bool {
    let claim_token_set = claim_tokens
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !claim_token_set.contains("button") {
        return false;
    }

    if claim_token_set.iter().any(|token| {
        matches!(
            *token,
            "sign"
                | "login"
                | "submit"
                | "email"
                | "password"
                | "verification"
                | "code"
                | "credential"
        )
    }) {
        return false;
    }

    if top_support.iter().any(|candidate| {
        matches!(
            candidate.block.kind,
            SnapshotBlockKind::Form | SnapshotBlockKind::Input
        )
    }) {
        return false;
    }

    let meaningful_claim_tokens = claim_tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.as_str(),
                "button" | "contain" | "contains" | "includ" | "page"
            ) && !token.chars().all(|character| character.is_ascii_digit())
        })
        .collect::<Vec<_>>();

    if meaningful_claim_tokens.is_empty() {
        return true;
    }

    let corroborating_non_button = top_support.iter().any(|candidate| {
        !matches!(candidate.block.kind, SnapshotBlockKind::Button)
            && meaningful_claim_tokens.iter().any(|claim_token| {
                tokenize_significant(&block_search_text(candidate.block))
                    .iter()
                    .any(|token| tokens_match(claim_token, token))
            })
    });

    !corroborating_non_button
}

pub(crate) fn assess_support_guards(
    claim: &ClaimRequest,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    claim_anchor_tokens: &[String],
    claim_qualifier_tokens: &[String],
) -> GuardAssessment {
    let aggregated_text = aggregate_support_text(top_support, blocks);
    let aggregated_tokens = tokenize_significant(&aggregated_text);
    let aggregated_all_tokens = tokenize_all(&aggregated_text);
    let normalized_claim = normalize_text(&claim.statement);
    let normalized_support = normalize_text(&aggregated_text);
    let mut guard_failures = Vec::new();
    let mut contradiction_reason = None;
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        anchor_guard_check(claim_anchor_tokens, &aggregated_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        qualifier_guard_check(claim_qualifier_tokens, &aggregated_all_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        numeric_guard_check(&claim.statement, &aggregated_text),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        scope_guard_check(&claim.statement, &aggregated_all_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        status_guard_check(&claim.statement, &aggregated_all_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        negation_guard_check(&normalized_claim, &normalized_support),
    );

    GuardAssessment {
        contradiction_reason: contradiction_reason.clone(),
        next_action_hint: if contradiction_reason.is_none() {
            next_action_hint_for_failures(&guard_failures)
        } else {
            None
        },
        guard_failures,
    }
}

fn apply_guard_check(
    contradiction_reason: &mut Option<UnsupportedClaimReason>,
    guard_failures: &mut Vec<EvidenceGuardFailure>,
    check: GuardCheck,
) {
    if let Some(reason) = check.contradiction_reason {
        contradiction_reason.get_or_insert(reason);
    }
    if let Some(failure) = check.failure {
        guard_failures.push(failure);
    }
}

fn anchor_guard_check(claim_anchor_tokens: &[String], aggregated_tokens: &[String]) -> GuardCheck {
    let anchor_coverage = if claim_anchor_tokens.is_empty() {
        1.0
    } else {
        token_overlap_ratio(claim_anchor_tokens, aggregated_tokens)
    };
    let required = required_anchor_coverage(claim_anchor_tokens.len());

    if anchor_coverage + 0.001 >= required {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::AnchorCoverage,
            detail: format!(
                "anchor coverage {:.2} is below required {:.2}",
                anchor_coverage, required
            ),
        }),
    }
}

fn qualifier_guard_check(
    claim_qualifier_tokens: &[String],
    aggregated_all_tokens: &[String],
) -> GuardCheck {
    let qualifier_coverage = if claim_qualifier_tokens.is_empty() {
        1.0
    } else {
        token_overlap_ratio(claim_qualifier_tokens, aggregated_all_tokens)
    };

    if qualifier_coverage >= 1.0 {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::QualifierCoverage,
            detail: format!(
                "qualifier coverage {:.2} is below required 1.00",
                qualifier_coverage
            ),
        }),
    }
}

fn numeric_guard_check(claim_text: &str, aggregated_text: &str) -> GuardCheck {
    let claim_numeric = numeric_expressions(claim_text);
    let support_numeric = numeric_expressions(aggregated_text);

    if claim_numeric.is_empty() {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    if support_numeric.is_empty() {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::NumericValue,
                detail: "No exact numeric detail was found in the retrieved support.".to_string(),
            }),
        };
    }

    if numeric_expressions_match(&claim_numeric, &support_numeric) {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: Some(UnsupportedClaimReason::NumericMismatch),
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::NumericValue,
            detail: format!(
                "Claim numeric values {:?} do not match support values {:?}.",
                claim_numeric
                    .iter()
                    .map(NumericExpression::render)
                    .collect::<Vec<_>>(),
                support_numeric
                    .iter()
                    .map(NumericExpression::render)
                    .collect::<Vec<_>>()
            ),
        }),
    }
}

fn scope_guard_check(claim_text: &str, aggregated_all_tokens: &[String]) -> GuardCheck {
    let claim_scope = detect_scope_profile(&tokenize_all(claim_text));
    let support_scope = detect_scope_profile(aggregated_all_tokens);

    if scope_profiles_contradict(claim_scope, support_scope) {
        return GuardCheck {
            contradiction_reason: Some(UnsupportedClaimReason::ScopeMismatch),
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Scope,
                detail: format!(
                    "Claim scope `{}` conflicts with support scope `{}`.",
                    claim_scope.label(),
                    support_scope.label()
                ),
            }),
        };
    }

    if claim_scope.requires_explicit_support() && support_scope == ScopeProfile::Unknown {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Scope,
                detail: "The claim requires explicit scope confirmation, but the support is scope-ambiguous.".to_string(),
            }),
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: None,
    }
}

fn status_guard_check(claim_text: &str, aggregated_all_tokens: &[String]) -> GuardCheck {
    let claim_status = detect_status_profile(&tokenize_all(claim_text));
    let support_status = detect_status_profile(aggregated_all_tokens);

    if status_profiles_contradict(claim_status, support_status) {
        return GuardCheck {
            contradiction_reason: Some(UnsupportedClaimReason::StatusMismatch),
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Status,
                detail: format!(
                    "Claim status `{}` conflicts with support status `{}`.",
                    claim_status.label(),
                    support_status.label()
                ),
            }),
        };
    }

    if claim_status.requires_explicit_support() && support_status == StatusProfile::Unknown {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Status,
                detail: "The claim requires explicit release-status support, but the support is status-ambiguous.".to_string(),
            }),
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: None,
    }
}

fn negation_guard_check(normalized_claim: &str, normalized_support: &str) -> GuardCheck {
    if !contradiction_detected(normalized_claim, normalized_support) {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: Some(UnsupportedClaimReason::NegationMismatch),
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::Negation,
            detail:
                "The retrieved support contains polarity language that conflicts with the claim."
                    .to_string(),
        }),
    }
}

fn append_unique_block_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    block: &SnapshotBlock,
) {
    if seen_blocks.insert(block.id.clone()) {
        parts.push(block_search_text(block));
    }
}

fn append_primary_heading_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
) {
    if let Some(primary_heading) = primary_heading_context(blocks) {
        append_unique_block_text(seen_blocks, parts, primary_heading);
    }
}

fn append_heading_context_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
    block: &SnapshotBlock,
) {
    if let Some(heading) = nearest_heading_context(blocks, block) {
        append_unique_block_text(seen_blocks, parts, heading);
    }
}

fn append_candidate_support_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
    candidate: &ScoredCandidate<'_>,
) {
    append_unique_block_text(seen_blocks, parts, candidate.block);
    append_heading_context_text(seen_blocks, parts, blocks, candidate.block);
}

fn aggregate_support_text(top_support: &[ScoredCandidate<'_>], blocks: &[SnapshotBlock]) -> String {
    let mut seen_blocks = BTreeSet::new();
    let mut parts = Vec::new();

    append_primary_heading_text(&mut seen_blocks, &mut parts, blocks);

    for candidate in top_support {
        append_candidate_support_text(&mut seen_blocks, &mut parts, blocks, candidate);
    }

    parts.join(" ")
}

fn aggregate_support_score(
    claim: &ClaimRequest,
    normalized_claim: &str,
    claim_tokens: &[String],
    claim_numeric_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    scoring_context: &ScoringContext,
) -> f64 {
    let claim_anchor_tokens = anchor_tokens(&tokenize_significant(&claim.statement));
    let narrative_support_count = top_support
        .iter()
        .filter(|candidate| is_narrative_aggregate_block(candidate.block))
        .count();
    let relevant_primary_heading = primary_heading_context(blocks)
        .filter(|heading| primary_heading_supports_claim(heading, &claim_anchor_tokens));

    if narrative_support_count < 2
        && !(narrative_support_count >= 1 && relevant_primary_heading.is_some())
    {
        return 0.0;
    }

    let aggregated_text = aggregate_support_text(top_support, blocks);
    if aggregated_text.is_empty() {
        return 0.0;
    }

    let aggregated_tokens = tokenize_significant(&aggregated_text);
    if aggregated_tokens.is_empty() {
        return 0.0;
    }

    let lexical_overlap = weighted_token_overlap_ratio(
        claim_tokens,
        &aggregated_tokens,
        &scoring_context.claim_token_weights,
    );
    let exact_bonus = exact_match_bonus(normalized_claim, &normalize_text(&aggregated_text));
    let numeric_overlap = numeric_overlap_ratio(claim_numeric_tokens, &aggregated_text);
    let title_bonus = relevant_primary_heading.map(|_| 0.04).unwrap_or(0.0);
    let distributed_support_bonus =
        distributed_support_bonus(&claim.statement, top_support, scoring_context);
    let multi_block_context_bonus =
        multi_block_context_bonus(narrative_support_count, relevant_primary_heading.is_some());
    let support_density_bonus = support_density_bonus(top_support, narrative_support_count);

    ((lexical_overlap * 0.76)
        + (exact_bonus * 0.14)
        + (numeric_overlap * 0.06)
        + title_bonus
        + multi_block_context_bonus
        + support_density_bonus
        + distributed_support_bonus)
        .min(1.0)
}

fn distributed_support_bonus(
    claim_text: &str,
    top_support: &[ScoredCandidate<'_>],
    scoring_context: &ScoringContext,
) -> f64 {
    if top_support.len() < 2 {
        return 0.0;
    }

    let claim_anchor_tokens = anchor_tokens(&tokenize_significant(claim_text));
    if claim_anchor_tokens.len() < 2 {
        return 0.0;
    }

    let mut covered_anchor_tokens = BTreeSet::new();
    let mut supporting_blocks = 0usize;

    for candidate in top_support {
        let block_tokens = tokenize_significant(&block_search_text(candidate.block));
        let matched = claim_anchor_tokens
            .iter()
            .filter(|claim_token| {
                block_tokens
                    .iter()
                    .any(|block_token| tokens_match(claim_token, block_token))
            })
            .cloned()
            .collect::<BTreeSet<_>>();

        if !matched.is_empty() {
            supporting_blocks += 1;
            covered_anchor_tokens.extend(matched);
        }
    }

    if supporting_blocks < 2 {
        return 0.0;
    }

    let coverage = weighted_anchor_coverage(
        &claim_anchor_tokens,
        &covered_anchor_tokens,
        &scoring_context.claim_token_weights,
    );

    if coverage >= 0.8 {
        0.14
    } else if coverage >= 0.6 {
        0.10
    } else {
        0.0
    }
}

fn multi_block_context_bonus(
    narrative_support_count: usize,
    has_relevant_primary_heading: bool,
) -> f64 {
    if narrative_support_count >= 2 && has_relevant_primary_heading {
        0.06
    } else if narrative_support_count >= 2 {
        0.03
    } else {
        0.0
    }
}

fn support_density_bonus(
    top_support: &[ScoredCandidate<'_>],
    narrative_support_count: usize,
) -> f64 {
    if narrative_support_count < 2 {
        return 0.0;
    }

    top_support
        .iter()
        .map(|candidate| candidate.score)
        .sum::<f64>()
        .min(1.0)
        * 0.24
}

fn weighted_anchor_coverage(
    claim_anchor_tokens: &[String],
    covered_anchor_tokens: &BTreeSet<String>,
    claim_token_weights: &BTreeMap<String, f64>,
) -> f64 {
    if claim_anchor_tokens.is_empty() {
        return 0.0;
    }

    let total_weight = claim_anchor_tokens
        .iter()
        .map(|token| claim_token_weight(token, claim_token_weights))
        .sum::<f64>();

    if total_weight <= f64::EPSILON {
        return 0.0;
    }

    let matched_weight = claim_anchor_tokens
        .iter()
        .filter(|token| covered_anchor_tokens.contains(*token))
        .map(|token| claim_token_weight(token, claim_token_weights))
        .sum::<f64>();

    matched_weight / total_weight
}

fn primary_heading_supports_claim(heading: &SnapshotBlock, claim_anchor_tokens: &[String]) -> bool {
    if claim_anchor_tokens.is_empty() {
        return false;
    }

    let heading_tokens = tokenize_significant(&heading.text);
    !heading_tokens.is_empty() && token_overlap_ratio(claim_anchor_tokens, &heading_tokens) >= 0.5
}

fn required_anchor_coverage(anchor_count: usize) -> f64 {
    match anchor_count {
        0 => 0.0,
        1 | 2 => 1.0,
        3 => 2.0 / 3.0,
        _ => 0.6,
    }
}

fn should_keep_browsing(
    best_score: f64,
    assessment: &GuardAssessment,
    claim: &ClaimRequest,
) -> bool {
    best_score >= 0.35
        && (!assessment.guard_failures.is_empty()
            || !numeric_expressions(&claim.statement).is_empty()
            || detect_scope_profile(&tokenize_all(&claim.statement)).requires_explicit_support()
            || detect_status_profile(&tokenize_all(&claim.statement)).requires_explicit_support())
}

fn numeric_expressions(text: &str) -> Vec<NumericExpression> {
    let tokens = normalize_text(text)
        .split_whitespace()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if token.chars().all(|character| character.is_ascii_digit()) {
            let expression = NumericExpression {
                value: token.clone(),
                unit: tokens
                    .get(index + 1)
                    .and_then(|candidate| normalize_unit(candidate)),
            };
            if !expressions.contains(&expression) {
                expressions.push(expression);
            }
        }
    }

    expressions
}

fn normalize_unit(token: &str) -> Option<String> {
    match token {
        "second" | "seconds" | "sec" | "secs" => Some("second".to_string()),
        "minute" | "minutes" | "min" | "mins" => Some("minute".to_string()),
        "hour" | "hours" | "hr" | "hrs" => Some("hour".to_string()),
        "day" | "days" => Some("day".to_string()),
        "week" | "weeks" => Some("week".to_string()),
        "month" | "months" => Some("month".to_string()),
        "year" | "years" => Some("year".to_string()),
        _ => None,
    }
}

fn numeric_expressions_match(
    claim_numeric: &[NumericExpression],
    support_numeric: &[NumericExpression],
) -> bool {
    claim_numeric.iter().all(|claim_expr| {
        support_numeric.iter().any(|support_expr| {
            claim_expr.value == support_expr.value
                && match (&claim_expr.unit, &support_expr.unit) {
                    (Some(claim_unit), Some(support_unit)) => claim_unit == support_unit,
                    _ => true,
                }
        })
    })
}

fn detect_scope_profile(tokens: &[String]) -> ScopeProfile {
    if tokens
        .iter()
        .any(|token| UNIVERSAL_SCOPE_TOKENS.contains(&token.as_str()))
    {
        ScopeProfile::Universal
    } else if tokens
        .iter()
        .any(|token| EXCLUSIVE_SCOPE_TOKENS.contains(&token.as_str()))
    {
        ScopeProfile::Exclusive
    } else if tokens
        .iter()
        .any(|token| LIMITED_SCOPE_TOKENS.contains(&token.as_str()))
    {
        ScopeProfile::Limited
    } else {
        ScopeProfile::Unknown
    }
}

fn scope_profiles_contradict(claim: ScopeProfile, support: ScopeProfile) -> bool {
    matches!(
        (claim, support),
        (ScopeProfile::Universal, ScopeProfile::Limited)
            | (ScopeProfile::Universal, ScopeProfile::Exclusive)
            | (ScopeProfile::Exclusive, ScopeProfile::Limited)
    )
}

fn detect_status_profile(tokens: &[String]) -> StatusProfile {
    if tokens
        .iter()
        .any(|token| PREVIEW_STATUS_TOKENS.contains(&token.as_str()))
    {
        StatusProfile::Preview
    } else if tokens
        .iter()
        .any(|token| GA_STATUS_TOKENS.contains(&token.as_str()))
    {
        StatusProfile::GenerallyAvailable
    } else if tokens
        .iter()
        .any(|token| DEPRECATED_STATUS_TOKENS.contains(&token.as_str()))
    {
        StatusProfile::Deprecated
    } else {
        StatusProfile::Unknown
    }
}

fn status_profiles_contradict(claim: StatusProfile, support: StatusProfile) -> bool {
    matches!(
        (claim, support),
        (StatusProfile::GenerallyAvailable, StatusProfile::Preview)
            | (StatusProfile::Preview, StatusProfile::GenerallyAvailable)
            | (StatusProfile::GenerallyAvailable, StatusProfile::Deprecated)
            | (StatusProfile::Preview, StatusProfile::Deprecated)
    )
}

fn next_action_hint_for_failures(failures: &[EvidenceGuardFailure]) -> Option<String> {
    if failures.iter().any(|failure| {
        matches!(
            failure.kind,
            EvidenceGuardKind::NumericValue | EvidenceGuardKind::NumericUnit
        )
    }) {
        Some("Browse the limits, pricing, or quotas page before answering.".to_string())
    } else if failures
        .iter()
        .any(|failure| matches!(failure.kind, EvidenceGuardKind::Scope))
    {
        Some(
            "Browse the regional availability or feature-matrix page before answering.".to_string(),
        )
    } else if failures
        .iter()
        .any(|failure| matches!(failure.kind, EvidenceGuardKind::Status))
    {
        Some("Browse the release notes or feature status page before answering.".to_string())
    } else if failures.is_empty() {
        None
    } else {
        Some("Browse a more specific source page before answering.".to_string())
    }
}

const UNIVERSAL_SCOPE_TOKENS: &[&str] =
    &["all", "every", "global", "worldwide", "entire", "universal"];
const EXCLUSIVE_SCOPE_TOKENS: &[&str] = &["only", "exclusive", "solely"];
const LIMITED_SCOPE_TOKENS: &[&str] = &[
    "selected", "some", "certain", "regional", "region", "varies", "subset", "specific",
];
const PREVIEW_STATUS_TOKENS: &[&str] = &["preview", "beta", "alpha", "experimental", "prelaunch"];
const GA_STATUS_TOKENS: &[&str] = &["launched", "generally", "ga"];
const DEPRECATED_STATUS_TOKENS: &[&str] =
    &["deprecated", "legacy", "retired", "sunset", "unsupported"];
