use std::collections::BTreeSet;
use touch_browser_contracts::{
    EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceReport, EvidenceSource,
    CONTRACT_VERSION,
};

use crate::{analyzer::analyze_claim, ClaimRequest, EvidenceInput};

pub(crate) fn build_claim_outcome(
    input: &EvidenceInput,
    claim: &ClaimRequest,
) -> EvidenceClaimOutcome {
    let resolution = analyze_claim(claim, &input.snapshot.blocks);
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
        reason: resolution.reason,
        checked_block_refs: resolution.checked_refs,
        guard_failures: resolution.guard_failures,
        next_action_hint: resolution.next_action_hint,
        verification_verdict: None,
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
        SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument,
        SnapshotEvidence, SnapshotSource, SourceRisk, SourceType,
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
