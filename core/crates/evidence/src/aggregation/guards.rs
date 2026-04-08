use std::collections::BTreeSet;

use touch_browser_contracts::{
    EvidenceGuardFailure, EvidenceGuardKind, SnapshotBlock, UnsupportedClaimReason,
};

use super::support::aggregate_support_text;
use crate::{
    contradiction::contradiction_detected,
    normalization::{
        normalize_text, token_overlap_ratio, tokenize_all, tokenize_significant, tokens_match,
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
    best_score >= 0.35
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

fn required_anchor_coverage(anchor_count: usize) -> f64 {
    match anchor_count {
        0 => 0.0,
        1 | 2 => 1.0,
        3 => 2.0 / 3.0,
        _ => 0.6,
    }
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
