use std::collections::BTreeSet;
use touch_browser_contracts::{
    EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceConfidenceBand,
    EvidenceGuardFailure, EvidenceGuardKind, EvidenceMatchSignals, EvidenceReport, EvidenceSource,
    EvidenceSupportSnippet, UnsupportedClaimReason, CONTRACT_VERSION,
};

use crate::{
    analyzer::analyze_claim,
    contradiction::contradiction_detected,
    normalization::{anchor_tokens, normalize_text, token_overlap_ratio, tokenize_significant},
    semantic_matching::{has_strong_nli_contradiction, score_nli_pairs, NliScore},
    ClaimRequest, EvidenceInput,
};

pub(crate) fn build_claim_outcome(
    input: &EvidenceInput,
    claim: &ClaimRequest,
) -> EvidenceClaimOutcome {
    let resolution = analyze_claim(claim, &input.snapshot.blocks);
    let support_snippets = build_support_snippets(&resolution.support);
    let match_signals = resolution.support.first().map(build_match_signals);
    let confidence_band = resolution.confidence.map(confidence_band_for_score);
    let review_recommended = matches!(confidence_band, Some(EvidenceConfidenceBand::Review))
        || matches!(
            resolution.verdict,
            EvidenceClaimVerdict::NeedsMoreBrowsing | EvidenceClaimVerdict::InsufficientEvidence
        );
    let verdict_explanation = Some(build_verdict_explanation(
        resolution.verdict.clone(),
        confidence_band.clone(),
        resolution.reason.clone(),
        &support_snippets,
        resolution
            .guard_failures
            .first()
            .map(|failure| failure.detail.as_str()),
        resolution.next_action_hint.as_deref(),
    ));
    let citation =
        (resolution.verdict == EvidenceClaimVerdict::EvidenceSupported).then(|| EvidenceCitation {
            url: input.snapshot.source.source_url.clone(),
            retrieved_at: input.generated_at.clone(),
            source_type: input.snapshot.source.source_type.clone(),
            source_risk: input.source_risk.clone(),
            source_label: input
                .source_label
                .clone()
                .or_else(|| input.snapshot.source.title.clone()),
        });

    EvidenceClaimOutcome {
        version: CONTRACT_VERSION.to_string(),
        claim_id: claim.claim_id.clone(),
        statement: claim.statement.clone(),
        verdict: resolution.verdict,
        support: resolution
            .support
            .iter()
            .filter_map({
                let mut seen = BTreeSet::new();
                move |candidate| {
                    seen.insert(candidate.block.id.clone())
                        .then(|| candidate.block.id.clone())
                }
            })
            .collect(),
        support_score: resolution.confidence,
        citation,
        support_snippets,
        reason: resolution.reason,
        confidence_band,
        review_recommended,
        verdict_explanation,
        match_signals,
        checked_block_refs: resolution.checked_refs,
        guard_failures: resolution.guard_failures,
        next_action_hint: resolution.next_action_hint,
        verification_verdict: None,
    }
}

fn build_support_snippets(
    support: &[crate::scoring::ScoredCandidate<'_>],
) -> Vec<EvidenceSupportSnippet> {
    let mut seen = BTreeSet::new();
    support
        .iter()
        .filter(|candidate| seen.insert(candidate.block.id.clone()))
        .map(|candidate| EvidenceSupportSnippet {
            block_id: candidate.block.id.clone(),
            stable_ref: candidate.block.stable_ref.clone(),
            snippet: truncate_snippet(&candidate.text),
        })
        .collect()
}

fn build_match_signals(candidate: &crate::scoring::ScoredCandidate<'_>) -> EvidenceMatchSignals {
    EvidenceMatchSignals {
        block_id: candidate.block.id.clone(),
        stable_ref: candidate.block.stable_ref.clone(),
        block_kind: candidate.block.kind.clone(),
        exact_support: candidate.signals.exact_support,
        lexical_overlap: Some((candidate.signals.lexical_overlap * 100.0).round() / 100.0),
        contextual_overlap: Some((candidate.signals.contextual_overlap * 100.0).round() / 100.0),
        numeric_alignment: candidate
            .signals
            .numeric_alignment
            .map(|value| (value * 100.0).round() / 100.0),
        semantic_similarity: candidate
            .signals
            .semantic_similarity
            .map(|value| (value * 100.0).round() / 100.0),
        semantic_boost: candidate
            .signals
            .semantic_boost
            .map(|value| (value * 100.0).round() / 100.0),
        nli_entailment: candidate
            .signals
            .nli_entailment
            .map(|value| (value * 100.0).round() / 100.0),
        nli_contradiction: candidate
            .signals
            .nli_contradiction
            .map(|value| (value * 100.0).round() / 100.0),
    }
}

fn truncate_snippet(text: &str) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut snippet = String::new();
    for character in collapsed.chars() {
        if snippet.chars().count() >= 220 {
            snippet.push_str("...");
            return snippet;
        }
        snippet.push(character);
    }
    snippet
}

fn confidence_band_for_score(score: f64) -> EvidenceConfidenceBand {
    if score >= 0.92 {
        EvidenceConfidenceBand::High
    } else if score >= 0.82 {
        EvidenceConfidenceBand::Medium
    } else {
        EvidenceConfidenceBand::Review
    }
}

fn build_verdict_explanation(
    verdict: EvidenceClaimVerdict,
    confidence_band: Option<EvidenceConfidenceBand>,
    reason: Option<UnsupportedClaimReason>,
    support_snippets: &[EvidenceSupportSnippet],
    first_guard_detail: Option<&str>,
    next_action_hint: Option<&str>,
) -> String {
    match verdict {
        EvidenceClaimVerdict::EvidenceSupported => match confidence_band {
            Some(EvidenceConfidenceBand::High) => format!(
                "Matched direct support in {} page block(s). Review the attached snippets before reusing the claim.",
                support_snippets.len().max(1)
            ),
            Some(EvidenceConfidenceBand::Medium) => format!(
                "Matched plausible support in {} page block(s). Review the attached snippets if the claim is high impact.",
                support_snippets.len().max(1)
            ),
            Some(EvidenceConfidenceBand::Review) => format!(
                "Support is present but still borderline across {} page block(s). Review the attached snippets before reusing the claim.",
                support_snippets.len().max(1)
            ),
            None => "Matched support from the current page. Review the attached snippets before reusing the claim.".to_string(),
        },
        EvidenceClaimVerdict::Contradicted => first_guard_detail
            .map(|detail| format!("Retrieved evidence conflicts with the claim: {detail}"))
            .unwrap_or_else(|| {
                "Retrieved evidence conflicts with the claim on the current page.".to_string()
            }),
        EvidenceClaimVerdict::NeedsMoreBrowsing => next_action_hint
            .map(str::to_string)
            .or_else(|| {
                first_guard_detail.map(|detail| {
                    format!(
                        "The current page surfaced mixed or borderline evidence: {detail}"
                    )
                })
            })
            .unwrap_or_else(|| {
                "The current page surfaced plausible evidence, but it still needs a more specific source or manual review.".to_string()
            }),
        EvidenceClaimVerdict::InsufficientEvidence => match reason {
            Some(UnsupportedClaimReason::InsufficientConfidence) => {
                "The page did not surface strong enough support for this claim yet.".to_string()
            }
            Some(UnsupportedClaimReason::NoSupportingBlock) => {
                "The page did not surface a direct support block for this claim.".to_string()
            }
            _ => "The page did not surface enough direct evidence for this claim.".to_string(),
        },
    }
}

pub(crate) fn build_report(
    input: &EvidenceInput,
    claim_outcomes: Vec<EvidenceClaimOutcome>,
) -> EvidenceReport {
    let mut claim_outcomes = claim_outcomes;
    downgrade_conflicting_supported_claims(&mut claim_outcomes, score_nli_pairs);

    let mut report = EvidenceReport {
        version: CONTRACT_VERSION.to_string(),
        generated_at: input.generated_at.clone(),
        source: EvidenceSource {
            source_url: input.snapshot.source.source_url.clone(),
            source_type: input.snapshot.source.source_type.clone(),
            source_risk: input.source_risk.clone(),
            source_label: input
                .source_label
                .clone()
                .or_else(|| input.snapshot.source.title.clone()),
        },
        supported_claims: Vec::new(),
        contradicted_claims: Vec::new(),
        unsupported_claims: Vec::new(),
        needs_more_browsing_claims: Vec::new(),
        claim_outcomes,
        verification: None,
    };
    report.rebuild_claim_buckets();
    report
}

fn downgrade_conflicting_supported_claims<F>(
    claim_outcomes: &mut [EvidenceClaimOutcome],
    score_pairs: F,
) where
    F: Fn(&[(String, String)]) -> Option<Vec<NliScore>>,
{
    let supported_indices = claim_outcomes
        .iter()
        .enumerate()
        .filter_map(|(index, outcome)| {
            (outcome.verdict == EvidenceClaimVerdict::EvidenceSupported).then_some(index)
        })
        .collect::<Vec<_>>();
    if supported_indices.len() < 2 {
        return;
    }

    let mut conflicting_indices = BTreeSet::new();
    let mut directional_pairs = Vec::new();
    let mut directional_pair_indices = Vec::new();

    for (offset, left_index) in supported_indices.iter().enumerate() {
        for right_index in supported_indices.iter().skip(offset + 1) {
            let left_statement = claim_outcomes[*left_index].statement.clone();
            let right_statement = claim_outcomes[*right_index].statement.clone();
            if contradiction_detected(&normalize_text(&left_statement), &right_statement)
                || contradiction_detected(&normalize_text(&right_statement), &left_statement)
            {
                conflicting_indices.insert(*left_index);
                conflicting_indices.insert(*right_index);
                continue;
            }
            if !claim_pair_merits_nli_conflict_check(&left_statement, &right_statement) {
                continue;
            }

            directional_pairs.push((left_statement.clone(), right_statement.clone()));
            directional_pair_indices.push((*left_index, *right_index));
            directional_pairs.push((right_statement, left_statement));
            directional_pair_indices.push((*left_index, *right_index));
        }
    }

    if !directional_pairs.is_empty() {
        if let Some(scores) = score_pairs(&directional_pairs) {
            for ((left_index, right_index), score) in
                directional_pair_indices.iter().zip(scores.iter())
            {
                if has_strong_nli_contradiction(score) {
                    conflicting_indices.insert(*left_index);
                    conflicting_indices.insert(*right_index);
                }
            }
        }
    }

    for index in conflicting_indices {
        let outcome = &mut claim_outcomes[index];
        outcome.verdict = EvidenceClaimVerdict::NeedsMoreBrowsing;
        outcome.reason = Some(UnsupportedClaimReason::NeedsMoreBrowsing);
        outcome.confidence_band = Some(EvidenceConfidenceBand::Review);
        outcome.review_recommended = true;
        if !outcome.guard_failures.iter().any(|failure| {
            failure.kind == EvidenceGuardKind::Predicate
                && failure
                    .detail
                    .contains("another supported claim in the same extract")
        }) {
            outcome.guard_failures.push(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Predicate,
                detail:
                    "This claim conflicts semantically with another supported claim in the same extract."
                        .to_string(),
            });
        }
        outcome.next_action_hint = Some(
            "The extract surfaced another supported claim with conflicting meaning. Review the snippets or browse a more specific source before answering.".to_string(),
        );
        outcome.verdict_explanation = Some(
            "Multiple supported claims in the same extract conflict semantically. Review the snippets or browse a more specific source before reusing either claim.".to_string(),
        );
    }
}

fn claim_pair_merits_nli_conflict_check(left_statement: &str, right_statement: &str) -> bool {
    let left_anchors = anchor_tokens(&tokenize_significant(left_statement));
    let right_anchors = anchor_tokens(&tokenize_significant(right_statement));
    if left_anchors.is_empty() || right_anchors.is_empty() {
        return false;
    }

    token_overlap_ratio(&left_anchors, &right_anchors) >= 0.6
        || token_overlap_ratio(&right_anchors, &left_anchors) >= 0.6
}

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceConfidenceBand,
        EvidenceMatchSignals, EvidenceSupportSnippet, SnapshotBlock, SnapshotBlockKind,
        SnapshotBlockRole, SnapshotBudget, SnapshotDocument, SnapshotEvidence, SnapshotSource,
        SourceRisk, SourceType, CONTRACT_VERSION,
    };

    use super::{build_claim_outcome, build_report, downgrade_conflicting_supported_claims};
    use crate::semantic_matching::NliScore;
    use crate::{ClaimRequest, EvidenceInput};

    fn report_input() -> EvidenceInput {
        EvidenceInput::new(
            SnapshotDocument {
                version: "1.0.0".to_string(),
                stable_ref_version: "1".to_string(),
                source: SnapshotSource {
                    source_url: "https://example.com/docs".to_string(),
                    source_type: SourceType::Http,
                    title: Some("Example Docs".to_string()),
                },
                budget: SnapshotBudget {
                    requested_tokens: 512,
                    estimated_tokens: 24,
                    emitted_tokens: 24,
                    truncated: false,
                },
                blocks: vec![SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:intro".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Example Docs supports HTTP snapshots.".to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://example.com/docs".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                }],
            },
            vec![ClaimRequest::new(
                "c1",
                "Example Docs supports HTTP snapshots.",
            )],
            "2026-04-08T00:00:00+09:00",
            SourceRisk::Low,
            Some("Example Docs".to_string()),
        )
    }

    #[test]
    fn supported_claim_outcomes_attach_citation_from_source_metadata() {
        let input = report_input();
        let outcome = build_claim_outcome(&input, &input.claims[0]);

        assert_eq!(
            outcome
                .citation
                .as_ref()
                .expect("supported claim should include citation")
                .url,
            "https://example.com/docs"
        );
        assert_eq!(outcome.confidence_band, Some(EvidenceConfidenceBand::High));
        assert!(!outcome.review_recommended);
        assert_eq!(outcome.support_snippets.len(), 1);
        assert_eq!(outcome.support_snippets[0].block_id, "b1");
        assert_eq!(
            outcome
                .match_signals
                .as_ref()
                .expect("supported claim should expose match signals")
                .block_kind,
            SnapshotBlockKind::Text
        );
        assert!(
            outcome
                .verdict_explanation
                .as_deref()
                .is_some_and(|explanation| explanation.contains("Matched direct support")),
            "supported claim should expose a human-readable explanation"
        );
    }

    #[test]
    fn build_report_rebuilds_supported_and_unsupported_buckets() {
        let input = report_input();
        let outcome = build_claim_outcome(&input, &input.claims[0]);
        let report = build_report(&input, vec![outcome]);

        assert_eq!(report.supported_claims.len(), 1);
        assert_eq!(report.supported_claims[0].support_snippets.len(), 1);
        assert!(report.unsupported_claims.is_empty());
        assert!(report.contradicted_claims.is_empty());
    }

    #[test]
    fn conflicting_supported_claim_pairs_are_downgraded_to_needs_more_browsing() {
        let mut outcomes = vec![
            supported_outcome("c1", "Rust uses a garbage collector."),
            supported_outcome("c2", "Rust has no garbage collector."),
        ];

        downgrade_conflicting_supported_claims(&mut outcomes, |_pairs| {
            Some(vec![
                NliScore {
                    contradiction: 0.98,
                    entailment: 0.01,
                    neutral: 0.01,
                },
                NliScore {
                    contradiction: 0.98,
                    entailment: 0.01,
                    neutral: 0.01,
                },
            ])
        });

        assert!(outcomes.iter().all(|outcome| {
            outcome.verdict == EvidenceClaimVerdict::NeedsMoreBrowsing
                && outcome.reason
                    == Some(touch_browser_contracts::UnsupportedClaimReason::NeedsMoreBrowsing)
        }));
    }

    #[test]
    fn claim_pair_nli_conflict_check_skips_low_overlap_claims() {
        let mut outcomes = vec![
            supported_outcome(
                "c1",
                "The Core research suite reported citation precision of 97%.",
            ),
            supported_outcome("c2", "The Hostile review suite reported recall of 91%."),
        ];

        downgrade_conflicting_supported_claims(&mut outcomes, |_pairs| {
            Some(vec![
                NliScore {
                    contradiction: 0.98,
                    entailment: 0.01,
                    neutral: 0.01,
                },
                NliScore {
                    contradiction: 0.98,
                    entailment: 0.01,
                    neutral: 0.01,
                },
            ])
        });

        assert!(outcomes
            .iter()
            .all(|outcome| outcome.verdict == EvidenceClaimVerdict::EvidenceSupported));
    }

    fn supported_outcome(claim_id: &str, statement: &str) -> EvidenceClaimOutcome {
        EvidenceClaimOutcome {
            version: CONTRACT_VERSION.to_string(),
            claim_id: claim_id.to_string(),
            statement: statement.to_string(),
            verdict: EvidenceClaimVerdict::EvidenceSupported,
            support: vec!["b1".to_string()],
            support_score: Some(0.95),
            citation: Some(EvidenceCitation {
                url: "https://example.com/docs".to_string(),
                retrieved_at: "2026-04-08T00:00:00+09:00".to_string(),
                source_type: SourceType::Http,
                source_risk: SourceRisk::Low,
                source_label: Some("Example Docs".to_string()),
            }),
            support_snippets: vec![EvidenceSupportSnippet {
                block_id: "b1".to_string(),
                stable_ref: "rmain:text:intro".to_string(),
                snippet: "Example Docs supports HTTP snapshots.".to_string(),
            }],
            reason: None,
            confidence_band: Some(EvidenceConfidenceBand::High),
            review_recommended: false,
            verdict_explanation: Some(
                "Matched direct support in 1 page block(s). Review the attached snippets before reusing the claim."
                    .to_string(),
            ),
            match_signals: Some(EvidenceMatchSignals {
                block_id: "b1".to_string(),
                stable_ref: "rmain:text:intro".to_string(),
                block_kind: SnapshotBlockKind::Text,
                exact_support: true,
                lexical_overlap: Some(1.0),
                contextual_overlap: Some(1.0),
                numeric_alignment: None,
                semantic_similarity: None,
                semantic_boost: None,
                nli_entailment: None,
                nli_contradiction: None,
            }),
            checked_block_refs: vec!["rmain:text:intro".to_string()],
            guard_failures: Vec::new(),
            next_action_hint: None,
            verification_verdict: None,
        }
    }
}
