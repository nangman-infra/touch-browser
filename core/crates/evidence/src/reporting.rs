use std::collections::BTreeSet;
use touch_browser_contracts::{
    EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceConfidenceBand,
    EvidenceMatchSignals, EvidenceReport, EvidenceSource, EvidenceSupportSnippet,
    UnsupportedClaimReason, CONTRACT_VERSION,
};

use crate::{analyzer::analyze_claim, ClaimRequest, EvidenceInput};

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

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        EvidenceConfidenceBand, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole,
        SnapshotBudget, SnapshotDocument, SnapshotEvidence, SnapshotSource, SourceRisk, SourceType,
    };

    use super::{build_claim_outcome, build_report};
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
        assert!(report.unsupported_claims.is_empty());
        assert!(report.contradicted_claims.is_empty());
    }
}
