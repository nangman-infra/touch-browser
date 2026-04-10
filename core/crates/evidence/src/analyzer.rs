use touch_browser_contracts::{
    EvidenceClaimVerdict, EvidenceGuardFailure, SnapshotBlock, UnsupportedClaimReason,
};

use crate::{
    aggregation::{
        assess_support_guards, checked_refs, contradiction_resolution, contradictory_support,
        effective_support_score, guarded_resolution, no_candidate_resolution,
        no_top_support_resolution, non_contradictory_candidates, support_acceptance_threshold,
        supported_resolution, top_support_candidates, SupportDecisionContext,
    },
    normalization::{build_claim_analysis_input, claim_is_low_signal_noise},
    scoring::{
        build_scoring_context, document_prefers_cross_lingual_matching, score_candidates,
        ScoredCandidate,
    },
    semantic_matching::{rerank_candidates_with_nli, rerank_candidates_with_semantic},
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
    if claim_is_low_signal_noise(&claim.statement, &analysis.claim_sequence_tokens) {
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
    let matching_profile = matching_profile_for_document(blocks, &analysis);
    let guard_claim_tokens = guard_claim_tokens(&analysis, &matching_profile);
    let scoring_context = build_scoring_context(blocks, matching_profile.claim_tokens);
    let mut scored = score_candidates(
        blocks,
        &analysis.normalized_claim,
        matching_profile.claim_tokens,
        &analysis.claim_qualifier_tokens,
        &analysis.claim_numeric_tokens,
        &scoring_context,
    );
    rerank_candidates_with_semantic(&claim.statement, &mut scored);
    rerank_candidates_with_nli(&claim.statement, &mut scored);
    let checked_refs = checked_refs(&scored);
    let contradictory_support = contradictory_support(&scored);
    let contradictory_exists = scored.iter().any(|candidate| candidate.contradictory);
    let non_contradictory = non_contradictory_candidates(scored);
    let best_non_contradictory_score = non_contradictory
        .first()
        .map(|candidate| candidate.score)
        .unwrap_or(0.0);
    let best_contradictory_score = contradictory_support
        .iter()
        .map(|candidate| candidate.score)
        .fold(0.0, f64::max);

    let Some(best_candidate) = non_contradictory.first() else {
        if let Some(resolution) =
            contradiction_resolution(claim, &contradictory_support, blocks, &analysis)
        {
            return resolution;
        }
        return no_candidate_resolution(contradictory_exists, checked_refs);
    };
    if best_contradictory_score >= best_non_contradictory_score + 0.02 {
        if let Some(resolution) =
            contradiction_resolution(claim, &contradictory_support, blocks, &analysis)
        {
            return resolution;
        }
    }
    let best_score = best_candidate.score;
    let top_support = top_support_candidates(non_contradictory);
    if top_support.is_empty() {
        return no_top_support_resolution(contradictory_exists, checked_refs);
    }

    let assessment = assess_support_guards(
        claim,
        &top_support,
        blocks,
        &analysis.claim_sequence_tokens,
        &guard_claim_tokens,
        matching_profile.claim_anchor_tokens,
        &analysis.claim_qualifier_tokens,
    );
    let effective_score = effective_support_score(
        &analysis,
        matching_profile.claim_tokens,
        matching_profile.claim_anchor_tokens,
        &top_support,
        blocks,
        &scoring_context,
        best_score,
    );
    let support_threshold = support_acceptance_threshold(
        &top_support,
        &assessment,
        matching_profile.uses_cross_lingual_matching,
    );
    if assessment.contradiction_reason.is_none()
        && !assessment.guard_failures.is_empty()
        && !contradictory_support.is_empty()
    {
        if let Some(resolution) =
            contradiction_resolution(claim, &contradictory_support, blocks, &analysis)
        {
            return resolution;
        }
    }

    if let Some(resolution) = guarded_resolution(
        claim,
        matching_profile.claim_tokens,
        &top_support,
        &checked_refs,
        &assessment,
        SupportDecisionContext {
            effective_score,
            support_threshold,
            uses_cross_lingual_matching: matching_profile.uses_cross_lingual_matching,
        },
    ) {
        return resolution;
    }

    supported_resolution(effective_score, top_support, checked_refs)
}

fn guard_claim_tokens(
    analysis: &crate::normalization::ClaimAnalysisInput,
    matching_profile: &MatchingProfile<'_>,
) -> Vec<String> {
    let mut combined = matching_profile.claim_tokens.to_vec();

    for token in &analysis.claim_cross_lingual_tokens {
        if !combined.contains(token) {
            combined.push(token.clone());
        }
    }

    combined
}

struct MatchingProfile<'a> {
    claim_tokens: &'a [String],
    claim_anchor_tokens: &'a [String],
    uses_cross_lingual_matching: bool,
}

fn matching_profile_for_document<'a>(
    blocks: &[SnapshotBlock],
    analysis: &'a crate::normalization::ClaimAnalysisInput,
) -> MatchingProfile<'a> {
    if analysis.claim_contains_cjk
        && !analysis.claim_cross_lingual_tokens.is_empty()
        && document_prefers_cross_lingual_matching(blocks)
    {
        MatchingProfile {
            claim_tokens: &analysis.claim_cross_lingual_tokens,
            claim_anchor_tokens: &analysis.claim_cross_lingual_anchor_tokens,
            uses_cross_lingual_matching: true,
        }
    } else {
        MatchingProfile {
            claim_tokens: &analysis.claim_tokens,
            claim_anchor_tokens: &analysis.claim_anchor_tokens,
            uses_cross_lingual_matching: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        EvidenceClaimVerdict, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole,
        SnapshotDocument, SnapshotEvidence, SnapshotSource, SourceRisk, SourceType,
        UnsupportedClaimReason,
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
        assert!(
            resolution
                .next_action_hint
                .as_deref()
                .is_some_and(|hint| hint.contains("Rewrite the claim")),
            "expected low-signal claim to return rewrite guidance"
        );
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

    #[test]
    fn analyzer_rejects_thread_usage_claim_from_contrastive_os_thread_language() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://nodejs.org/en/about".to_string(),
                source_type: SourceType::Http,
                title: Some("About Node.js".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:contrast".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "This is in contrast to today's more common concurrency model, in which OS threads are employed.".to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://nodejs.org/en/about".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:threadless".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Node.js being designed without threads doesn't mean you can't take advantage of multiple cores in your environment."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://nodejs.org/en/about".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", "Node.js uses OS threads.")],
                "2026-04-10T00:00:00+09:00",
                SourceRisk::Low,
                Some("About Node.js".to_string()),
            ))
            .expect("extract should succeed");

        assert!(report.supported_claims.is_empty());
        assert!(
            report
                .contradicted_claims
                .iter()
                .any(|claim| claim.claim_id == "c1"),
            "thread-usage claim should be treated as contradicted when the page explicitly says Node.js is designed without threads"
        );
    }

    #[test]
    fn analyzer_contradicts_execution_model_claims_when_document_states_the_opposite() {
        let mut snapshot = simple_snapshot(
            "Python is an interpreted high-level general-purpose programming language.",
        );
        snapshot.blocks.push(SnapshotBlock {
            version: "1.0.0".to_string(),
            id: "b2".to_string(),
            kind: SnapshotBlockKind::List,
            stable_ref: "rmain:list:python-implementations".to_string(),
            role: SnapshotBlockRole::Content,
            text: "- CPython - PyPy - Jython".to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://example.com".to_string(),
                source_type: SourceType::Http,
                dom_path_hint: Some("html > body > main > ul".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        });

        let resolution = analyze_claim(
            &ClaimRequest::new("c1", "Python is a compiled language."),
            &snapshot.blocks,
        );

        assert_eq!(resolution.verdict, EvidenceClaimVerdict::Contradicted);
        assert_eq!(
            resolution.reason,
            Some(UnsupportedClaimReason::PredicateMismatch)
        );
    }

    #[test]
    fn analyzer_does_not_support_generic_supertype_blocks_for_execution_model_claims() {
        let mut snapshot = simple_snapshot("Python is a programming language.");
        snapshot.blocks.push(SnapshotBlock {
            version: "1.0.0".to_string(),
            id: "b2".to_string(),
            kind: SnapshotBlockKind::List,
            stable_ref: "rmain:list:python-libraries".to_string(),
            role: SnapshotBlockRole::Content,
            text: "- NumPy - pandas - Django".to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://example.com".to_string(),
                source_type: SourceType::Http,
                dom_path_hint: Some("html > body > main > ul".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        });

        let resolution = analyze_claim(
            &ClaimRequest::new("c1", "Python is a compiled language."),
            &snapshot.blocks,
        );

        assert_ne!(resolution.verdict, EvidenceClaimVerdict::EvidenceSupported);
        assert!(
            resolution.guard_failures.iter().any(|failure| {
                failure.kind == touch_browser_contracts::EvidenceGuardKind::Predicate
            }),
            "expected predicate guard failure for generic supertype support"
        );
    }

    #[test]
    fn analyzer_supports_korean_claims_against_english_evidence_with_cross_lingual_tokens() {
        let snapshot = simple_snapshot(
            "The Fetch API provides an interface for fetching resources, including across the network.",
        );

        let resolution = analyze_claim(
            &ClaimRequest::new(
                "c1",
                "Fetch API는 네트워크 요청을 위한 인터페이스를 제공한다.",
            ),
            &snapshot.blocks,
        );

        assert_eq!(resolution.verdict, EvidenceClaimVerdict::EvidenceSupported);
    }

    #[test]
    fn analyzer_supports_chinese_claims_against_english_evidence_with_cross_lingual_tokens() {
        let snapshot = simple_snapshot(
            "The Fetch API provides an interface for fetching resources, including across the network.",
        );

        let resolution = analyze_claim(
            &ClaimRequest::new("c1", "Fetch API 提供了用于网络请求的接口。"),
            &snapshot.blocks,
        );

        assert_eq!(resolution.verdict, EvidenceClaimVerdict::EvidenceSupported);
    }

    #[test]
    fn analyzer_requires_more_browsing_for_conflicting_threading_model_evidence() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://example.com/tokio".to_string(),
                source_type: SourceType::Http,
                title: Some("Tokio Runtime Modes".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:current-thread-runtime".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Tokio provides a current-thread runtime for lightweight bridging scenarios."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://example.com/tokio".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:multi-thread-runtime".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Tokio also offers a multi-threaded runtime for concurrent workloads."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://example.com/tokio".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let resolution = analyze_claim(
            &ClaimRequest::new("c1", "Tokio is single-threaded."),
            &snapshot.blocks,
        );

        assert_ne!(resolution.verdict, EvidenceClaimVerdict::EvidenceSupported);
        assert!(
            resolution.reason == Some(UnsupportedClaimReason::PredicateMismatch)
                || resolution.guard_failures.iter().any(|failure| {
                    failure.kind == touch_browser_contracts::EvidenceGuardKind::Predicate
                }),
            "expected threading-model predicate rejection"
        );

        let multi_threaded = analyze_claim(
            &ClaimRequest::new("c2", "Tokio is multi-threaded."),
            &snapshot.blocks,
        );

        assert_ne!(
            multi_threaded.verdict,
            EvidenceClaimVerdict::EvidenceSupported
        );
    }

    #[test]
    fn analyzer_requires_explicit_default_qualifier_support_for_threading_claims() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://example.com/tokio".to_string(),
                source_type: SourceType::Http,
                title: Some("Tokio Runtime Modes".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:current-thread-runtime".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Tokio provides a current-thread runtime for lightweight bridging scenarios."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://example.com/tokio".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:multi-thread-runtime".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Tokio also offers a multi-threaded runtime for concurrent workloads."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://example.com/tokio".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let resolution = analyze_claim(
            &ClaimRequest::new("c1", "Tokio is single-threaded by default."),
            &snapshot.blocks,
        );

        assert_ne!(resolution.verdict, EvidenceClaimVerdict::EvidenceSupported);
    }

    #[test]
    fn analyzer_requires_more_browsing_for_mixed_runtime_sentences_inside_one_block() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://example.com/tokio".to_string(),
                source_type: SourceType::Http,
                title: Some("Tokio Runtime Modes".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: SnapshotBlockKind::Text,
                stable_ref: "rmain:text:runtime-modes".to_string(),
                role: SnapshotBlockRole::Content,
                text: "Tokio provides a current-thread runtime for lightweight bridging scenarios, but it also offers a multi-threaded runtime for concurrent workloads.".to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://example.com/tokio".to_string(),
                    source_type: SourceType::Http,
                    dom_path_hint: Some("html > body > main > p".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let single_threaded = analyze_claim(
            &ClaimRequest::new("c1", "Tokio is single-threaded by default."),
            &snapshot.blocks,
        );
        assert_ne!(
            single_threaded.verdict,
            EvidenceClaimVerdict::EvidenceSupported
        );

        let multi_threaded = analyze_claim(
            &ClaimRequest::new("c2", "Tokio is multi-threaded."),
            &snapshot.blocks,
        );
        assert_ne!(
            multi_threaded.verdict,
            EvidenceClaimVerdict::EvidenceSupported
        );
    }
}
