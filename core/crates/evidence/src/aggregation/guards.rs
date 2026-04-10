use std::collections::BTreeSet;

use touch_browser_contracts::{
    EvidenceGuardFailure, EvidenceGuardKind, SnapshotBlock, SnapshotBlockKind,
    UnsupportedClaimReason,
};

use super::support::aggregate_support_text;
use crate::{
    contradiction::contradiction_detected,
    normalization::{
        claim_mentions_version_or_release, normalize_text, token_overlap_ratio, tokenize_all,
        tokenize_significant, tokens_match,
    },
    scoring::{block_search_text, ScoredCandidate},
    ClaimRequest,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PredicatePolarity {
    None,
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum QualifierProfile {
    Unknown,
    Default,
    Maximum,
    Minimum,
}

struct PredicateOpposition {
    label: &'static str,
    positive: &'static [&'static str],
    negative: &'static [&'static str],
    subordinate_targets: &'static [&'static str],
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

pub(super) fn assess_support_guards(
    claim: &ClaimRequest,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    claim_tokens: &[String],
    predicate_hint_tokens: &[String],
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
        numeric_guard_check(
            &claim.statement,
            &aggregated_text,
            top_support,
            claim_anchor_tokens,
        ),
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
        qualifier_guard_check(claim_qualifier_tokens, top_support, &claim.statement),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        thread_usage_guard_check(&claim.statement, top_support, blocks),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        predicate_guard_check(
            claim_tokens,
            predicate_hint_tokens,
            top_support,
            blocks,
            claim_anchor_tokens,
        ),
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

pub(super) fn button_claim_requires_more_browsing(
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
            touch_browser_contracts::SnapshotBlockKind::Form
                | touch_browser_contracts::SnapshotBlockKind::Input
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
        !matches!(
            candidate.block.kind,
            touch_browser_contracts::SnapshotBlockKind::Button
        ) && meaningful_claim_tokens.iter().any(|claim_token| {
            tokenize_significant(&block_search_text(candidate.block))
                .iter()
                .any(|token| tokens_match(claim_token, token))
        })
    });

    !corroborating_non_button
}

pub(super) fn should_keep_browsing(
    best_score: f64,
    assessment: &GuardAssessment,
    claim: &ClaimRequest,
) -> bool {
    best_score >= 0.30
        && (!assessment.guard_failures.is_empty()
            || !numeric_expressions(&claim.statement).is_empty()
            || detect_scope_profile(&tokenize_all(&claim.statement)).requires_explicit_support()
            || detect_status_profile(&tokenize_all(&claim.statement)).requires_explicit_support())
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
    top_support: &[ScoredCandidate<'_>],
    claim_text: &str,
) -> GuardCheck {
    let claim_qualifier = detect_qualifier_profile(claim_text);
    if claim_qualifier == QualifierProfile::Unknown {
        let qualifier_coverage = if claim_qualifier_tokens.is_empty() {
            1.0
        } else {
            top_support
                .iter()
                .map(|candidate| {
                    token_overlap_ratio(claim_qualifier_tokens, &tokenize_all(&candidate.text))
                })
                .fold(0.0, f64::max)
        };
        return if qualifier_coverage < 1.0 {
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
        } else {
            GuardCheck {
                contradiction_reason: None,
                failure: None,
            }
        };
    }

    let support_qualifiers = top_support
        .iter()
        .map(|candidate| detect_qualifier_profile(qualifier_guard_text(candidate, claim_qualifier)))
        .filter(|profile| *profile != QualifierProfile::Unknown)
        .collect::<BTreeSet<_>>();

    if support_qualifiers.contains(&claim_qualifier) {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    let qualifier_coverage = if claim_qualifier_tokens.is_empty() {
        1.0
    } else {
        top_support
            .iter()
            .map(|candidate| {
                token_overlap_ratio(
                    claim_qualifier_tokens,
                    &tokenize_all(qualifier_guard_text(candidate, claim_qualifier)),
                )
            })
            .fold(0.0, f64::max)
    };
    if qualifier_coverage < 1.0 {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::QualifierCoverage,
                detail: format!(
                    "qualifier coverage {:.2} is below required 1.00",
                    qualifier_coverage
                ),
            }),
        };
    }

    if let Some(support_qualifier) = support_qualifiers.iter().next().copied() {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Predicate,
                detail: format!(
                    "Claim qualifier `{}` conflicts with support qualifier `{}`.",
                    claim_qualifier.label(),
                    support_qualifier.label()
                ),
            }),
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::Predicate,
            detail: format!(
                "The claim requires explicit `{}` evidence, but the retrieved support is qualifier-ambiguous.",
                claim_qualifier.label()
            ),
        }),
    }
}

fn numeric_guard_check(
    claim_text: &str,
    aggregated_text: &str,
    top_support: &[ScoredCandidate<'_>],
    claim_anchor_tokens: &[String],
) -> GuardCheck {
    if claim_mentions_version_or_release(&tokenize_significant(claim_text))
        || claim_contains_raw_version_marker(claim_text)
    {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    let claim_numeric = numeric_expressions(claim_text);
    let claim_qualifier = detect_qualifier_profile(claim_text);
    let claim_requires_unit = claim_numeric
        .iter()
        .any(|expression| expression.unit.is_some());
    let qualifier_matched_candidates = if claim_qualifier == QualifierProfile::Unknown {
        Vec::new()
    } else {
        top_support
            .iter()
            .filter(|candidate| {
                detect_qualifier_profile(numeric_guard_text(candidate, claim_qualifier))
                    == claim_qualifier
            })
            .collect::<Vec<_>>()
    };
    let qualifier_matched_narrative_candidates = qualifier_matched_candidates
        .iter()
        .copied()
        .filter(|candidate| matches!(candidate.block.kind, SnapshotBlockKind::Text))
        .collect::<Vec<_>>();
    let all_candidates = top_support.iter().collect::<Vec<_>>();
    let primary_candidates = if !qualifier_matched_narrative_candidates.is_empty() {
        qualifier_matched_narrative_candidates
    } else if qualifier_matched_candidates.is_empty() {
        all_candidates.clone()
    } else {
        qualifier_matched_candidates
    };
    let mut numeric_evidence_candidates = select_anchor_aligned_numeric_candidates(
        &primary_candidates,
        claim_anchor_tokens,
        claim_qualifier,
        claim_requires_unit,
    );
    let allow_global_numeric_fallback = !matches!(claim_qualifier, QualifierProfile::Default);
    if numeric_evidence_candidates.is_empty()
        && allow_global_numeric_fallback
        && primary_candidates.len() != all_candidates.len()
    {
        numeric_evidence_candidates = select_anchor_aligned_numeric_candidates(
            &all_candidates,
            claim_anchor_tokens,
            claim_qualifier,
            claim_requires_unit,
        );
    }
    if numeric_evidence_candidates.is_empty() {
        numeric_evidence_candidates = primary_candidates;
    }
    let qualifier_matched_text = numeric_evidence_candidates
        .iter()
        .map(|candidate| numeric_guard_text(candidate, claim_qualifier))
        .collect::<Vec<_>>()
        .join(" ");
    let support_numeric = if qualifier_matched_text.is_empty() {
        numeric_expressions(aggregated_text)
    } else {
        numeric_expressions(&qualifier_matched_text)
    };

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

    if !numeric_mismatch_is_hard_contradiction(
        &numeric_evidence_candidates,
        claim_numeric.len(),
        &support_numeric,
        claim_requires_unit,
    ) {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::NumericValue,
                detail: format!(
                    "Claim numeric values {:?} do not align with support values {:?}, but the mismatch comes from summary or table-like support rather than an explicit narrative contradiction.",
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

fn numeric_mismatch_is_hard_contradiction(
    top_support: &[&ScoredCandidate<'_>],
    claim_numeric_count: usize,
    support_numeric: &[NumericExpression],
    claim_requires_unit: bool,
) -> bool {
    if top_support.is_empty() {
        return false;
    }

    if claim_requires_unit
        && !support_numeric
            .iter()
            .any(|expression| expression.unit.is_some())
    {
        return false;
    }

    let has_precise_numeric_support = top_support.iter().any(|candidate| {
        matches!(
            candidate.block.kind,
            SnapshotBlockKind::Text
                | SnapshotBlockKind::Heading
                | SnapshotBlockKind::Table
                | SnapshotBlockKind::List
                | SnapshotBlockKind::Metadata
        ) && numeric_expressions(&candidate.block.text).len()
            <= claim_numeric_count.saturating_add(3)
    });
    let support_numeric_is_dense = support_numeric.len() > claim_numeric_count.saturating_add(4);

    has_precise_numeric_support && !support_numeric_is_dense
}

fn select_anchor_aligned_numeric_candidates<'a>(
    candidates: &[&'a ScoredCandidate<'a>],
    claim_anchor_tokens: &[String],
    claim_qualifier: QualifierProfile,
    require_unit: bool,
) -> Vec<&'a ScoredCandidate<'a>> {
    let anchor_aligned = candidates
        .iter()
        .copied()
        .filter(|candidate| {
            claim_anchor_tokens.is_empty()
                || token_overlap_ratio(
                    claim_anchor_tokens,
                    &tokenize_significant(numeric_guard_text(candidate, claim_qualifier)),
                ) > 0.0
        })
        .collect::<Vec<_>>();

    if anchor_aligned.is_empty() {
        return Vec::new();
    }

    if !require_unit {
        return anchor_aligned;
    }

    let unit_aligned = anchor_aligned
        .iter()
        .copied()
        .filter(|candidate| {
            numeric_expressions(numeric_guard_text(candidate, claim_qualifier))
                .iter()
                .any(|expression| expression.unit.is_some())
        })
        .collect::<Vec<_>>();

    if unit_aligned.is_empty() {
        Vec::new()
    } else {
        unit_aligned
    }
}

fn claim_contains_raw_version_marker(text: &str) -> bool {
    text.split_whitespace()
        .map(|token| {
            token.trim_matches(|character: char| {
                character == ',' || character == '.' || character == ';' || character == ':'
            })
        })
        .any(raw_token_looks_like_version)
}

fn raw_token_looks_like_version(token: &str) -> bool {
    let normalized = token.trim();
    let candidate = normalized.strip_prefix('v').unwrap_or(normalized);
    candidate
        .chars()
        .filter(|character| *character == '.')
        .count()
        >= 1
        && candidate
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
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

fn predicate_guard_check(
    claim_tokens: &[String],
    predicate_hint_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    claim_anchor_tokens: &[String],
) -> GuardCheck {
    for opposition in PREDICATE_OPPOSITIONS {
        let predicate_source_tokens =
            if claim_predicate_polarity(claim_tokens, opposition).is_some() {
                claim_tokens
            } else {
                predicate_hint_tokens
            };
        let Some(claim_polarity) = claim_predicate_polarity(predicate_source_tokens, opposition)
        else {
            continue;
        };
        let claim_predicate_terms =
            claim_specific_predicate_terms(predicate_source_tokens, opposition, claim_polarity);
        let claim_context_tokens = {
            let raw_context = predicate_context_tokens(claim_tokens, &claim_predicate_terms);
            if raw_context.is_empty() {
                predicate_context_tokens(predicate_hint_tokens, &claim_predicate_terms)
            } else {
                raw_context
            }
        };
        if opposition.label == "execution-model"
            && !claim_context_tokens
                .iter()
                .any(|token| is_execution_model_context_token(token))
        {
            continue;
        }
        let subject_anchor_tokens =
            predicate_subject_anchor_tokens(claim_anchor_tokens, opposition);
        let matching_blocks = support_blocks_with_matching_predicate(
            top_support,
            opposition,
            claim_polarity,
            &claim_predicate_terms,
            &claim_context_tokens,
        );
        let opposite_blocks = document_blocks_with_opposite_predicate(
            blocks,
            &subject_anchor_tokens,
            claim_polarity,
            opposition,
            &claim_context_tokens,
        );

        if let Some(check) =
            predicate_guard_resolution(opposition, &matching_blocks, &opposite_blocks)
        {
            return check;
        }
    }

    GuardCheck {
        contradiction_reason: None,
        failure: None,
    }
}

fn thread_usage_guard_check(
    claim_text: &str,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
) -> GuardCheck {
    let claim_tokens = tokenize_significant(claim_text);
    if !claim_explicitly_mentions_thread_usage(&claim_tokens) {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    let subject_tokens = thread_usage_subject_tokens(&claim_tokens);
    if subject_tokens.is_empty() {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    let matching_blocks = top_support
        .iter()
        .filter(|candidate| {
            block_explicitly_supports_thread_usage(candidate.block, &subject_tokens)
        })
        .map(|candidate| candidate.block.id.clone())
        .collect::<BTreeSet<_>>();
    let opposite_blocks = blocks
        .iter()
        .filter(|block| block_explicitly_denies_thread_usage(block, &subject_tokens))
        .map(|block| block.id.clone())
        .collect::<BTreeSet<_>>();

    predicate_guard_resolution(&THREAD_USAGE_OPPOSITION, &matching_blocks, &opposite_blocks)
        .unwrap_or(GuardCheck {
            contradiction_reason: None,
            failure: None,
        })
}

fn claim_predicate_polarity(
    claim_tokens: &[String],
    opposition: &PredicateOpposition,
) -> Option<PredicatePolarity> {
    let polarity = detect_predicate_polarity(claim_tokens, opposition);
    (polarity != PredicatePolarity::None).then_some(polarity)
}

fn support_blocks_with_matching_predicate(
    top_support: &[ScoredCandidate<'_>],
    opposition: &PredicateOpposition,
    claim_polarity: PredicatePolarity,
    claim_predicate_terms: &[&str],
    claim_context_tokens: &[String],
) -> BTreeSet<String> {
    top_support
        .iter()
        .filter_map(|candidate| {
            let search_text = block_search_text(candidate.block);
            let candidate_tokens = normalized_sequence_tokens(&search_text);
            let candidate_polarity = detect_predicate_polarity(&candidate_tokens, opposition);
            if candidate_polarity != claim_polarity {
                return None;
            }
            if !block_explicitly_asserts_predicate(
                &candidate_tokens,
                opposition,
                claim_polarity,
                claim_predicate_terms,
                claim_context_tokens,
            ) {
                return None;
            }
            Some(candidate.block.id.clone())
        })
        .collect()
}

fn predicate_guard_resolution(
    opposition: &PredicateOpposition,
    matching_blocks: &BTreeSet<String>,
    opposite_blocks: &BTreeSet<String>,
) -> Option<GuardCheck> {
    if !matching_blocks.is_empty() {
        let mixed_polarity = opposite_blocks
            .iter()
            .any(|block_id| !matching_blocks.contains(block_id));
        return mixed_polarity.then(|| GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Predicate,
                detail: format!(
                    "The document contains both sides of `{}` for the same subject, so this claim needs a more specific source.",
                    opposition.label
                ),
            }),
        });
    }

    if !opposite_blocks.is_empty() {
        return Some(GuardCheck {
            contradiction_reason: Some(UnsupportedClaimReason::PredicateMismatch),
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Predicate,
                detail: format!(
                    "Document evidence contains the opposite `{}` predicate for the same subject.",
                    opposition.label
                ),
            }),
        });
    }

    Some(GuardCheck {
        contradiction_reason: None,
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::Predicate,
            detail: format!(
                "The claim requires explicit `{}` evidence, but the retrieved support only matches broader context.",
                opposition.label
            ),
        }),
    })
}

fn required_anchor_coverage(anchor_count: usize) -> f64 {
    match anchor_count {
        0 => 0.0,
        1 | 2 => 1.0,
        3 => 2.0 / 3.0,
        _ => 0.6,
    }
}

fn claim_explicitly_mentions_thread_usage(tokens: &[String]) -> bool {
    contains_token_from_set(tokens, THREAD_USAGE_POSITIVE_VERBS)
        && contains_token_from_set(tokens, THREAD_USAGE_OBJECT_TOKENS)
}

fn thread_usage_subject_tokens(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .filter(|token| {
            !contains_token(token, THREAD_USAGE_POSITIVE_VERBS)
                && !contains_token(token, THREAD_USAGE_OBJECT_TOKENS)
                && !contains_token(token, THREAD_USAGE_CONTEXT_NOISE_TOKENS)
        })
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn block_explicitly_supports_thread_usage(
    block: &SnapshotBlock,
    subject_tokens: &[String],
) -> bool {
    let search_text = block_search_text(block);
    let tokens = tokenize_all(&search_text);
    subject_anchor_overlap(&search_text, subject_tokens) >= 0.5
        && contains_thread_usage_support(&tokens)
}

fn block_explicitly_denies_thread_usage(block: &SnapshotBlock, subject_tokens: &[String]) -> bool {
    let search_text = block_search_text(block);
    let tokens = tokenize_all(&search_text);
    subject_anchor_overlap(&search_text, subject_tokens) >= 0.5
        && contains_thread_usage_denial(&tokens)
}

fn subject_anchor_overlap(block_text: &str, subject_tokens: &[String]) -> f64 {
    if subject_tokens.is_empty() {
        return 0.0;
    }

    token_overlap_ratio(subject_tokens, &tokenize_significant(block_text))
}

fn contains_thread_usage_support(tokens: &[String]) -> bool {
    contains_nearby_token_pair(
        tokens,
        THREAD_USAGE_POSITIVE_VERBS,
        THREAD_USAGE_OBJECT_TOKENS,
        3,
    )
}

fn contains_thread_usage_denial(tokens: &[String]) -> bool {
    contains_token_sequence_from_terms(tokens, &["without", "threads"])
        || contains_token_sequence_from_terms(tokens, &["without", "thread"])
        || contains_token_sequence_from_terms(tokens, &["no", "threads"])
        || contains_token_sequence_from_terms(tokens, &["no", "thread"])
        || contains_token_sequence_from_terms(tokens, &["designed", "without", "threads"])
}

fn contains_nearby_token_pair(
    tokens: &[String],
    first_terms: &[&str],
    second_terms: &[&str],
    max_gap: usize,
) -> bool {
    tokens.iter().enumerate().any(|(index, token)| {
        contains_token(token, first_terms)
            && tokens
                .iter()
                .skip(index + 1)
                .take(max_gap)
                .any(|candidate| contains_token(candidate, second_terms))
    })
}

fn contains_token_sequence_from_terms(tokens: &[String], phrase: &[&str]) -> bool {
    if phrase.is_empty() || tokens.len() < phrase.len() {
        return false;
    }

    tokens.windows(phrase.len()).any(|window| {
        window
            .iter()
            .zip(phrase.iter())
            .all(|(token, expected)| tokens_match(token, expected))
    })
}

fn contains_token_from_set(tokens: &[String], expected_terms: &[&str]) -> bool {
    tokens
        .iter()
        .any(|token| contains_token(token, expected_terms))
}

fn contains_token(token: &str, expected_terms: &[&str]) -> bool {
    expected_terms
        .iter()
        .any(|expected| tokens_match(token, expected))
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
        } else if let Some(month_value) = month_number(token) {
            let expression = NumericExpression {
                value: month_value.to_string(),
                unit: Some("month".to_string()),
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

fn month_number(token: &str) -> Option<u8> {
    match token {
        "january" | "jan" => Some(1),
        "february" | "feb" => Some(2),
        "march" | "mar" => Some(3),
        "april" | "apr" => Some(4),
        "may" => Some(5),
        "june" | "jun" => Some(6),
        "july" | "jul" => Some(7),
        "august" | "aug" => Some(8),
        "september" | "sep" | "sept" => Some(9),
        "october" | "oct" => Some(10),
        "november" | "nov" => Some(11),
        "december" | "dec" => Some(12),
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

fn detect_qualifier_profile(text: &str) -> QualifierProfile {
    let normalized = normalize_text(text);
    let tokens = tokenize_all(&normalized);
    let token_set = tokens.iter().map(String::as_str).collect::<BTreeSet<_>>();

    let has_default = contains_phrase(&normalized, &["by", "default"])
        || contains_phrase(&normalized, &["default"])
        || token_set.contains("default");
    if has_default {
        return QualifierProfile::Default;
    }

    let has_maximum = contains_phrase(&normalized, &["at", "most"])
        || contains_phrase(&normalized, &["up", "to"])
        || token_set.contains("maximum")
        || token_set.contains("max");
    if has_maximum {
        return QualifierProfile::Maximum;
    }

    let has_minimum = contains_phrase(&normalized, &["at", "least"])
        || token_set.contains("minimum")
        || token_set.contains("min");
    if has_minimum {
        return QualifierProfile::Minimum;
    }

    QualifierProfile::Unknown
}

fn contains_phrase(normalized_text: &str, phrase: &[&str]) -> bool {
    let tokens = normalized_text.split_whitespace().collect::<Vec<_>>();
    if phrase.is_empty() || tokens.len() < phrase.len() {
        return false;
    }

    tokens.windows(phrase.len()).any(|window| window == phrase)
}

fn qualifier_guard_text<'a>(
    candidate: &'a ScoredCandidate<'_>,
    claim_qualifier: QualifierProfile,
) -> &'a str {
    if claim_qualifier != QualifierProfile::Unknown
        && detect_qualifier_profile(&candidate.text) == QualifierProfile::Unknown
    {
        &candidate.block.text
    } else {
        &candidate.text
    }
}

fn numeric_guard_text<'a>(
    candidate: &'a ScoredCandidate<'_>,
    claim_qualifier: QualifierProfile,
) -> &'a str {
    qualifier_guard_text(candidate, claim_qualifier)
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

fn detect_predicate_polarity(
    tokens: &[String],
    opposition: &PredicateOpposition,
) -> PredicatePolarity {
    let has_positive = tokens_match_opposition_terms(tokens, opposition.positive);
    let has_negative = tokens_match_opposition_terms(tokens, opposition.negative);

    match (has_positive, has_negative) {
        (true, false) => PredicatePolarity::Positive,
        (false, true) => PredicatePolarity::Negative,
        _ => PredicatePolarity::None,
    }
}

fn predicate_subject_anchor_tokens(
    claim_anchor_tokens: &[String],
    opposition: &PredicateOpposition,
) -> Vec<String> {
    let filtered = claim_anchor_tokens
        .iter()
        .filter(|token| {
            !opposition_matches_token(opposition.positive, token)
                && !opposition_matches_token(opposition.negative, token)
        })
        .cloned()
        .collect::<Vec<_>>();

    if filtered.is_empty() {
        claim_anchor_tokens.to_vec()
    } else {
        filtered
    }
}

fn opposition_matches_token(opposition_terms: &[&str], token: &str) -> bool {
    opposition_terms.iter().any(|term| {
        token == *term || token.starts_with(term) || token.replace('-', "") == term.replace('-', "")
    })
}

fn tokens_match_opposition_terms(tokens: &[String], opposition_terms: &[&str]) -> bool {
    opposition_terms
        .iter()
        .any(|term| predicate_term_position(tokens, term).is_some())
}

fn predicate_term_position(tokens: &[String], term: &str) -> Option<usize> {
    let term_tokens = normalized_sequence_tokens(term);
    if term_tokens.is_empty() || tokens.len() < term_tokens.len() {
        return None;
    }

    tokens.windows(term_tokens.len()).position(|window| {
        window
            .iter()
            .zip(term_tokens.iter())
            .all(|(token, expected)| tokens_match(token, expected))
    })
}

fn predicate_polarities_conflict(claim: PredicatePolarity, support: PredicatePolarity) -> bool {
    matches!(
        (claim, support),
        (PredicatePolarity::Positive, PredicatePolarity::Negative)
            | (PredicatePolarity::Negative, PredicatePolarity::Positive)
    )
}

fn document_blocks_with_opposite_predicate(
    blocks: &[SnapshotBlock],
    claim_anchor_tokens: &[String],
    claim_polarity: PredicatePolarity,
    opposition: &PredicateOpposition,
    claim_context_tokens: &[String],
) -> BTreeSet<String> {
    let required_anchor_overlap = if claim_anchor_tokens.is_empty() {
        0.0
    } else {
        0.5
    };

    blocks
        .iter()
        .filter_map(|block| {
            let search_text = block_search_text(block);
            let block_tokens = normalized_sequence_tokens(&search_text);
            let block_polarity = detect_predicate_polarity(&block_tokens, opposition);
            if !predicate_polarities_conflict(claim_polarity, block_polarity) {
                return None;
            }
            if !block_explicitly_asserts_predicate(
                &block_tokens,
                opposition,
                opposite_polarity(claim_polarity),
                predicate_terms_for_polarity(opposition, opposite_polarity(claim_polarity)),
                claim_context_tokens,
            ) {
                return None;
            }

            if claim_anchor_tokens.is_empty() {
                return Some(block.id.clone());
            }

            let block_significant_tokens = tokenize_significant(&search_text);
            (token_overlap_ratio(claim_anchor_tokens, &block_significant_tokens)
                >= required_anchor_overlap)
                .then(|| block.id.clone())
        })
        .collect()
}

fn opposite_polarity(polarity: PredicatePolarity) -> PredicatePolarity {
    match polarity {
        PredicatePolarity::Positive => PredicatePolarity::Negative,
        PredicatePolarity::Negative => PredicatePolarity::Positive,
        PredicatePolarity::None => PredicatePolarity::None,
    }
}

fn predicate_terms_for_polarity(
    opposition: &PredicateOpposition,
    polarity: PredicatePolarity,
) -> &[&str] {
    match polarity {
        PredicatePolarity::Positive => opposition.positive,
        PredicatePolarity::Negative => opposition.negative,
        PredicatePolarity::None => &[],
    }
}

fn claim_specific_predicate_terms<'a>(
    claim_tokens: &[String],
    opposition: &'a PredicateOpposition,
    polarity: PredicatePolarity,
) -> Vec<&'a str> {
    let polarity_terms = predicate_terms_for_polarity(opposition, polarity);
    let specific_terms = polarity_terms
        .iter()
        .copied()
        .filter(|term| predicate_term_position(claim_tokens, term).is_some())
        .collect::<Vec<_>>();

    if specific_terms.is_empty() {
        polarity_terms.to_vec()
    } else {
        specific_terms
    }
}

fn block_explicitly_asserts_predicate(
    block_tokens: &[String],
    opposition: &PredicateOpposition,
    polarity: PredicatePolarity,
    predicate_terms: &[&str],
    claim_context_tokens: &[String],
) -> bool {
    if predicate_terms.is_empty() {
        return false;
    }

    let Some(predicate_index) = predicate_terms
        .iter()
        .filter_map(|term| predicate_term_position(block_tokens, term))
        .min()
    else {
        return false;
    };

    if predicate_has_subordinate_target(block_tokens, predicate_index, opposition) {
        return false;
    }

    if claim_context_tokens.is_empty() {
        return true;
    }

    predicate_context_matches(claim_context_tokens, block_tokens, predicate_terms)
        || execution_model_language_context_matches(
            opposition,
            polarity,
            claim_context_tokens,
            block_tokens,
            predicate_index,
        )
}

fn predicate_has_subordinate_target(
    block_tokens: &[String],
    predicate_index: usize,
    opposition: &PredicateOpposition,
) -> bool {
    if opposition.subordinate_targets.is_empty() {
        return false;
    }

    block_tokens
        .iter()
        .skip(predicate_index + 1)
        .take(4)
        .any(|token| contains_token(token, opposition.subordinate_targets))
}

fn predicate_context_tokens(tokens: &[String], opposition_terms: &[&str]) -> Vec<String> {
    for (index, token) in tokens.iter().enumerate() {
        if !opposition_matches_token(opposition_terms, token) {
            continue;
        }

        let context = tokens[index + 1..]
            .iter()
            .filter_map(|token| normalize_predicate_context_token(token))
            .filter(|token| !PREDICATE_CONTEXT_STOP_WORDS.contains(&token.as_str()))
            .take(10)
            .collect::<Vec<_>>();
        if !context.is_empty() {
            return context;
        }
    }

    Vec::new()
}

fn predicate_context_matches(
    claim_context_tokens: &[String],
    support_tokens: &[String],
    support_predicate_terms: &[&str],
) -> bool {
    if claim_context_tokens.is_empty() {
        return true;
    }

    let support_context_tokens = predicate_context_tokens(support_tokens, support_predicate_terms);
    !support_context_tokens.is_empty()
        && token_overlap_ratio(claim_context_tokens, &support_context_tokens) >= 0.5
}

fn normalize_predicate_context_token(token: &str) -> Option<String> {
    if token.is_empty() {
        return None;
    }

    let normalized = if contains_hangul(token) {
        strip_hangul_context_suffix(token)
    } else {
        token.to_string()
    };

    (!normalized.is_empty()).then_some(normalized)
}

fn normalized_sequence_tokens(text: &str) -> Vec<String> {
    normalize_text(text)
        .split_whitespace()
        .map(ToString::to_string)
        .collect()
}

fn execution_model_language_context_matches(
    opposition: &PredicateOpposition,
    polarity: PredicatePolarity,
    claim_context_tokens: &[String],
    block_tokens: &[String],
    predicate_index: usize,
) -> bool {
    if opposition.label != "execution-model" || polarity != PredicatePolarity::Negative {
        return false;
    }

    if !claim_context_tokens
        .iter()
        .all(|token| is_language_context_token(token))
    {
        return false;
    }

    block_tokens
        .iter()
        .skip(predicate_index + 1)
        .take(8)
        .any(|token| is_language_context_token(token))
}

fn is_language_context_token(token: &str) -> bool {
    LANGUAGE_CONTEXT_TOKENS
        .iter()
        .any(|candidate| tokens_match(token, candidate))
}

fn is_execution_model_context_token(token: &str) -> bool {
    EXECUTION_MODEL_CONTEXT_TOKENS
        .iter()
        .any(|candidate| tokens_match(token, candidate))
}

fn contains_hangul(text: &str) -> bool {
    text.chars()
        .any(|character| matches!(character as u32, 0xac00..=0xd7af))
}

fn strip_hangul_context_suffix(token: &str) -> String {
    for suffix in HANGUL_CONTEXT_SUFFIXES {
        if let Some(stripped) = token.strip_suffix(suffix) {
            if stripped.chars().count() >= 2 {
                return stripped.to_string();
            }
        }
    }

    token.to_string()
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
    } else if failures
        .iter()
        .any(|failure| matches!(failure.kind, EvidenceGuardKind::Predicate))
    {
        Some("Browse a source sentence that explicitly states the property you are claiming before answering.".to_string())
    } else if failures.is_empty() {
        None
    } else {
        Some("Browse a more specific source page before answering.".to_string())
    }
}

impl QualifierProfile {
    fn label(self) -> &'static str {
        match self {
            QualifierProfile::Unknown => "unknown",
            QualifierProfile::Default => "default",
            QualifierProfile::Maximum => "maximum",
            QualifierProfile::Minimum => "minimum",
        }
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
const THREAD_USAGE_POSITIVE_VERBS: &[&str] =
    &["use", "uses", "using", "employ", "employs", "employed"];
const THREAD_USAGE_OBJECT_TOKENS: &[&str] = &["thread", "threads"];
const THREAD_USAGE_CONTEXT_NOISE_TOKENS: &[&str] = &["os", "operating", "system"];
const THREAD_USAGE_OPPOSITION: PredicateOpposition = PredicateOpposition {
    label: "thread-usage",
    positive: &["uses threads"],
    negative: &["without threads"],
    subordinate_targets: &[],
};
const PREDICATE_OPPOSITIONS: &[PredicateOpposition] = &[
    PredicateOpposition {
        label: "execution-model",
        positive: &["compiled", "컴파일", "编译"],
        negative: &["interpreted", "interpret", "인터프리", "해석", "解释"],
        subordinate_targets: &["byte", "bytes", "bytecode", "code", "codes", "source"],
    },
    PredicateOpposition {
        label: "threading-model",
        positive: &[
            "single-threaded",
            "singlethreaded",
            "current-thread",
            "currentthread",
        ],
        negative: &[
            "multi-threaded",
            "multithreaded",
            "multi_thread",
            "multithread",
        ],
        subordinate_targets: &[],
    },
    PredicateOpposition {
        label: "runtime-behavior",
        positive: &["synchronous", "synchronou", "동기"],
        negative: &["asynchronous", "asynchronou", "비동기", "异步"],
        subordinate_targets: &[
            "method",
            "methods",
            "version",
            "versions",
            "callback",
            "callbacks",
            "operation",
            "operations",
            "library",
            "libraries",
            "api",
            "apis",
        ],
    },
];
const PREDICATE_CONTEXT_STOP_WORDS: &[&str] = &[
    "a", "an", "and", "as", "at", "be", "by", "for", "from", "in", "is", "it", "its", "of", "or",
    "the", "to", "with",
];
const HANGUL_CONTEXT_SUFFIXES: &[&str] = &[
    "으로부터",
    "으로써",
    "으로서",
    "에서는",
    "에서",
    "에게",
    "으로",
    "이다",
    "이자",
    "처럼",
    "하다",
    "하는",
    "하며",
    "하고",
    "한",
    "는",
    "은",
    "이",
    "가",
    "을",
    "를",
    "에",
    "의",
    "로",
];
const LANGUAGE_CONTEXT_TOKENS: &[&str] = &["language", "languages", "언어", "语言"];
const EXECUTION_MODEL_CONTEXT_TOKENS: &[&str] = &[
    "language",
    "languages",
    "runtime",
    "runtimes",
    "model",
    "models",
    "언어",
    "语言",
    "런타임",
    "모델",
];

#[cfg(test)]
mod tests {
    use super::{
        block_explicitly_asserts_predicate, normalized_sequence_tokens, numeric_expressions,
        numeric_expressions_match, predicate_context_tokens, PredicateOpposition,
        PredicatePolarity,
    };

    #[test]
    fn numeric_expressions_match_korean_numeric_dates_with_english_month_names() {
        let claim_numeric =
            numeric_expressions("최초의 유인 달 착륙은 1969년 7월 20일 아폴로 11호였다.");
        let support_numeric = numeric_expressions(
            "The missions spanned a 41-month period starting 20 July 1969, beginning with Apollo 11.",
        );

        assert!(
            numeric_expressions_match(&claim_numeric, &support_numeric),
            "expected Korean numeric date tokens to align with English month-name dates"
        );
    }

    #[test]
    fn execution_model_guard_accepts_korean_interpreter_language_support() {
        let opposition = PredicateOpposition {
            label: "execution-model",
            positive: &["compiled", "컴파일", "编译"],
            negative: &["interpreted", "interpret", "인터프리", "해석", "解释"],
            subordinate_targets: &["byte", "bytes", "bytecode", "code", "codes", "source"],
        };
        let claim_tokens = normalized_sequence_tokens("파이썬은 인터프리터 언어이다.");
        let claim_context = predicate_context_tokens(&claim_tokens, &["인터프리"]);
        let block_tokens = normalized_sequence_tokens(
            "파이썬은 인터프리터를 사용하는 객체지향 언어이자 플랫폼에 독립적인 언어다.",
        );

        assert!(
            block_explicitly_asserts_predicate(
                &block_tokens,
                &opposition,
                PredicatePolarity::Negative,
                &["인터프리"],
                &claim_context,
            ),
            "expected explicit interpreter-language phrasing to satisfy the execution-model predicate guard"
        );
    }
}
