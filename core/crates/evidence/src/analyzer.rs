use touch_browser_contracts::{
    EvidenceClaimVerdict, EvidenceGuardFailure, SnapshotBlock, UnsupportedClaimReason,
};

use crate::{
    aggregation::{
        assess_support_guards, checked_refs, contradiction_resolution, contradictory_support,
        effective_support_score, guarded_resolution, no_candidate_resolution,
        no_top_support_resolution, non_contradictory_candidates, support_acceptance_threshold,
        supported_resolution, top_support_candidates,
    },
    normalization::{build_claim_analysis_input, claim_is_low_signal_noise},
    scoring::{build_scoring_context, score_candidates, ScoredCandidate},
    ClaimRequest,
};

#[derive(Debug)]
pub(crate) struct ClaimResolution<'a> {
    pub(crate) verdict: EvidenceClaimVerdict,
    pub(crate) support: Vec<ScoredCandidate<'a>>,
    pub(crate) confidence: Option<f64>,
    pub(crate) reason: Option<UnsupportedClaimReason>,
    pub(crate) checked_refs: Vec<String>,
    pub(crate) guard_failures: Vec<EvidenceGuardFailure>,
    pub(crate) next_action_hint: Option<String>,
}

pub(crate) fn analyze_claim<'a>(
    claim: &ClaimRequest,
    blocks: &'a [SnapshotBlock],
) -> ClaimResolution<'a> {
    let analysis = build_claim_analysis_input(claim);
    if claim_is_low_signal_noise(&claim.statement, &analysis.claim_tokens) {
        return ClaimResolution {
            verdict: EvidenceClaimVerdict::InsufficientEvidence,
            support: Vec::new(),
            confidence: None,
            reason: Some(UnsupportedClaimReason::InsufficientConfidence),
            checked_refs: Vec::new(),
            guard_failures: Vec::new(),
            next_action_hint: Some(
                "Rewrite the claim into a shorter, source-checkable statement before answering."
                    .to_string(),
            ),
        };
    }
    let scoring_context = build_scoring_context(blocks, &analysis.claim_tokens);
    let scored = score_candidates(blocks, &analysis, &scoring_context);
    let checked_refs = checked_refs(&scored);
    let contradictory_support = contradictory_support(&scored);

    if let Some(resolution) =
        contradiction_resolution(claim, &contradictory_support, blocks, &analysis)
    {
        return resolution;
    }

    let contradictory_exists = scored.iter().any(|candidate| candidate.contradictory);
    let non_contradictory = non_contradictory_candidates(scored);

    let Some(best_candidate) = non_contradictory.first() else {
        return no_candidate_resolution(contradictory_exists, checked_refs);
    };
    let best_score = best_candidate.score;
    let top_support = top_support_candidates(non_contradictory);

    if top_support.is_empty() {
        return no_top_support_resolution(contradictory_exists, checked_refs);
    }

    let assessment = assess_support_guards(
        claim,
        &top_support,
        blocks,
        &analysis.claim_anchor_tokens,
        &analysis.claim_qualifier_tokens,
    );
    let effective_score = effective_support_score(
        claim,
        &analysis,
        &top_support,
        blocks,
        &scoring_context,
        best_score,
    );
    let support_threshold = support_acceptance_threshold(&top_support, &assessment);

    if let Some(resolution) = guarded_resolution(
        claim,
        &analysis,
        &top_support,
        &checked_refs,
        &assessment,
        effective_score,
        support_threshold,
    ) {
        return resolution;
    }

    supported_resolution(best_score, top_support, checked_refs)
}

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotDocument, SnapshotEvidence,
        SnapshotSource, SourceRisk, SourceType, UnsupportedClaimReason,
    };

    use super::analyze_claim;
    use crate::{ClaimRequest, EvidenceExtractor, EvidenceInput};

    fn simple_snapshot(text: &str) -> SnapshotDocument {
        SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://example.com".to_string(),
                source_type: SourceType::Http,
                title: Some("Example".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 16,
                emitted_tokens: 16,
                truncated: false,
            },
            blocks: vec![SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: SnapshotBlockKind::Text,
                stable_ref: "rmain:text:intro".to_string(),
                role: SnapshotBlockRole::Content,
                text: text.to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://example.com".to_string(),
                    source_type: SourceType::Http,
                    dom_path_hint: Some("html > body > main > p".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        }
    }

    #[test]
    fn analyzer_rejects_repetitive_long_claims() {
        let noisy_claim = ClaimRequest::new(
            "c1",
            format!("파이썬은 {}좋은 언어이다", "매우 ".repeat(200)),
        );
        let snapshot = simple_snapshot("Python is a programming language.");
        let resolution = analyze_claim(&noisy_claim, &snapshot.blocks);

        assert_eq!(
            resolution.reason,
            Some(UnsupportedClaimReason::InsufficientConfidence)
        );
        assert!(resolution.support.is_empty());
    }

    #[test]
    fn analyzer_supports_direct_exact_match_claims() {
        let snapshot = simple_snapshot("Python is a programming language.");
        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", "Python is a programming language.")],
                "2026-04-08T00:00:00+09:00",
                SourceRisk::Low,
                Some("Example".to_string()),
            ))
            .expect("extract should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.unsupported_claims.is_empty());
    }
}
