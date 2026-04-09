mod guards;
mod support;

use touch_browser_contracts::{
    EvidenceClaimVerdict, EvidenceGuardFailure, EvidenceGuardKind, SnapshotBlock,
    UnsupportedClaimReason,
};

use crate::{
    analyzer::ClaimResolution,
    normalization::ClaimAnalysisInput,
    scoring::{is_narrative_aggregate_block, round_confidence, ScoredCandidate, ScoringContext},
    ClaimRequest,
};

use guards::{button_claim_requires_more_browsing, should_keep_browsing, GuardAssessment};
use support::aggregate_support_score;

pub(crate) fn checked_refs(scored: &[ScoredCandidate<'_>]) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut refs = Vec::new();

    for candidate in scored {
        if seen.insert(candidate.block.stable_ref.clone()) {
            refs.push(candidate.block.stable_ref.clone());
            if refs.len() == 3 {
                break;
            }
        }
    }

    refs
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

pub(crate) fn assess_support_guards(
    claim: &ClaimRequest,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    claim_anchor_tokens: &[String],
    claim_qualifier_tokens: &[String],
) -> GuardAssessment {
    guards::assess_support_guards(
        claim,
        top_support,
        blocks,
        claim_anchor_tokens,
        claim_qualifier_tokens,
    )
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
    let eligible = non_contradictory
        .into_iter()
        .filter(|candidate| candidate.score >= 0.22)
        .collect::<Vec<_>>();

    let mut seen_blocks = std::collections::BTreeSet::new();
    let mut selected = Vec::new();

    for candidate in &eligible {
        if seen_blocks.insert(candidate.block.id.clone()) {
            selected.push(candidate.clone());
            if selected.len() == 3 {
                return selected;
            }
        }
    }

    let mut seen_candidates = selected
        .iter()
        .map(|candidate| (candidate.block.id.clone(), candidate.candidate_index))
        .collect::<std::collections::BTreeSet<_>>();

    for candidate in eligible {
        if seen_candidates.insert((candidate.block.id.clone(), candidate.candidate_index)) {
            selected.push(candidate);
            if selected.len() == 3 {
                break;
            }
        }
    }

    selected
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
    analysis: &ClaimAnalysisInput,
    claim_tokens: &[String],
    claim_anchor_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    scoring_context: &ScoringContext,
    best_score: f64,
) -> f64 {
    let aggregated_score = aggregate_support_score(
        &analysis.normalized_claim,
        claim_tokens,
        claim_anchor_tokens,
        &analysis.claim_numeric_tokens,
        top_support,
        blocks,
        scoring_context,
    );
    best_score.max(aggregated_score)
}

pub(crate) fn guarded_resolution<'a>(
    claim: &ClaimRequest,
    claim_tokens: &[String],
    top_support: &[ScoredCandidate<'a>],
    checked_refs: &[String],
    assessment: &GuardAssessment,
    decision: SupportDecisionContext,
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

    if decision.effective_score >= decision.support_threshold
        && assessment.guard_failures.is_empty()
    {
        if support_requires_additional_confirmation(
            decision.effective_score,
            top_support,
            decision.uses_cross_lingual_matching,
        ) {
            return Some(ClaimResolution {
                verdict: EvidenceClaimVerdict::NeedsMoreBrowsing,
                support: top_support.to_vec(),
                confidence: None,
                reason: Some(UnsupportedClaimReason::NeedsMoreBrowsing),
                checked_refs: checked_refs.to_vec(),
                guard_failures: Vec::new(),
                next_action_hint: Some(
                    "The retrieved support is plausible but still borderline. Inspect the source sentence directly before answering.".to_string(),
                ),
            });
        }
        if button_claim_requires_more_browsing(claim_tokens, top_support) {
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

    let verdict = if should_keep_browsing(decision.effective_score, assessment, claim) {
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
    effective_score: f64,
    top_support: Vec<ScoredCandidate<'a>>,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    let confidence = round_confidence(
        effective_score.max(
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

fn support_requires_additional_confirmation(
    effective_score: f64,
    top_support: &[ScoredCandidate<'_>],
    uses_cross_lingual_matching: bool,
) -> bool {
    if uses_cross_lingual_matching {
        return false;
    }

    let confidence = round_confidence(
        top_support
            .iter()
            .map(|candidate| candidate.score)
            .fold(effective_score, f64::max),
    );
    let has_exact_support = top_support.iter().any(|candidate| candidate.exact_support);
    let multi_block_support = top_support.len() >= 2;

    confidence < 0.82 && !has_exact_support && !multi_block_support
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct SupportDecisionContext {
    pub(crate) effective_score: f64,
    pub(crate) support_threshold: f64,
    pub(crate) uses_cross_lingual_matching: bool,
}

pub(crate) fn support_acceptance_threshold(
    top_support: &[ScoredCandidate<'_>],
    assessment: &GuardAssessment,
    uses_cross_lingual_matching: bool,
) -> f64 {
    if !assessment.guard_failures.is_empty() || top_support.len() < 2 {
        if uses_cross_lingual_matching {
            return 0.40;
        }
        return 0.52;
    }

    let narrative_support_count = top_support
        .iter()
        .filter(|candidate| is_narrative_aggregate_block(candidate.block))
        .map(|candidate| candidate.block.id.as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    if uses_cross_lingual_matching && narrative_support_count >= 2 {
        0.38
    } else if uses_cross_lingual_matching {
        0.40
    } else if narrative_support_count >= 2 {
        0.46
    } else {
        0.52
    }
}

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotEvidence, SourceType,
    };

    use super::support_requires_additional_confirmation;
    use crate::scoring::ScoredCandidate;

    fn candidate(score: f64, exact_support: bool) -> ScoredCandidate<'static> {
        let block = Box::leak(Box::new(SnapshotBlock {
            version: "1.0.0".to_string(),
            id: format!("b-{score}"),
            kind: SnapshotBlockKind::Text,
            stable_ref: "rmain:text:test".to_string(),
            role: SnapshotBlockRole::Content,
            text: "support block".to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://example.com".to_string(),
                source_type: SourceType::Http,
                dom_path_hint: Some("html > body > main > p".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        }));

        ScoredCandidate {
            block,
            candidate_index: 0,
            text: "support block".to_string(),
            score,
            lexical_overlap: score,
            contradictory: false,
            exact_support,
        }
    }

    #[test]
    fn weak_single_block_support_requires_more_confirmation() {
        let top_support = vec![candidate(0.58, false)];

        assert!(support_requires_additional_confirmation(
            0.58,
            &top_support,
            false
        ));
    }

    #[test]
    fn exact_support_does_not_require_more_confirmation() {
        let top_support = vec![candidate(0.58, true)];

        assert!(!support_requires_additional_confirmation(
            0.58,
            &top_support,
            false
        ));
    }

    #[test]
    fn multi_block_support_does_not_require_more_confirmation() {
        let top_support = vec![candidate(0.55, false), candidate(0.54, false)];

        assert!(!support_requires_additional_confirmation(
            0.55,
            &top_support,
            false
        ));
    }
}
