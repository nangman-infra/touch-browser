use std::collections::{BTreeMap, BTreeSet};

use touch_browser_contracts::{
    EvidenceCitation, SessionSynthesisClaim, SessionSynthesisClaimStatus, CONTRACT_VERSION,
};

use crate::ReadOnlySession;

#[derive(Debug, Clone)]
struct AggregateClaim {
    claim_id: String,
    statement: String,
    status: SessionSynthesisClaimStatus,
    snapshot_ids: BTreeSet<String>,
    support_refs: BTreeSet<String>,
    citations: Vec<EvidenceCitation>,
    citation_keys: BTreeSet<String>,
}

pub(crate) fn aggregate_session_claims(session: &ReadOnlySession) -> Vec<SessionSynthesisClaim> {
    let mut aggregates = BTreeMap::<String, AggregateClaim>::new();

    for evidence_record in &session.evidence_reports {
        for supported_claim in &evidence_record.report.supported_claims {
            let aggregate = ensure_aggregate(
                &mut aggregates,
                &supported_claim.claim_id,
                &supported_claim.statement,
                SessionSynthesisClaimStatus::EvidenceSupported,
            );
            record_snapshot_id(aggregate, &evidence_record.snapshot_id);
            record_support_refs(aggregate, &supported_claim.support);
            record_citation(aggregate, &supported_claim.citation);
        }

        for unsupported_claim in &evidence_record.report.contradicted_claims {
            let aggregate = ensure_aggregate(
                &mut aggregates,
                &unsupported_claim.claim_id,
                &unsupported_claim.statement,
                SessionSynthesisClaimStatus::Contradicted,
            );
            update_claim_status(aggregate, SessionSynthesisClaimStatus::Contradicted);
            record_snapshot_id(aggregate, &evidence_record.snapshot_id);
            record_support_refs(aggregate, &unsupported_claim.checked_block_refs);
        }

        for unsupported_claim in &evidence_record.report.unsupported_claims {
            let aggregate = ensure_aggregate(
                &mut aggregates,
                &unsupported_claim.claim_id,
                &unsupported_claim.statement,
                SessionSynthesisClaimStatus::InsufficientEvidence,
            );
            update_claim_status(aggregate, SessionSynthesisClaimStatus::InsufficientEvidence);
            record_snapshot_id(aggregate, &evidence_record.snapshot_id);
            record_support_refs(aggregate, &unsupported_claim.checked_block_refs);
        }

        for unsupported_claim in &evidence_record.report.needs_more_browsing_claims {
            let aggregate = ensure_aggregate(
                &mut aggregates,
                &unsupported_claim.claim_id,
                &unsupported_claim.statement,
                SessionSynthesisClaimStatus::NeedsMoreBrowsing,
            );
            update_claim_status(aggregate, SessionSynthesisClaimStatus::NeedsMoreBrowsing);
            record_snapshot_id(aggregate, &evidence_record.snapshot_id);
            record_support_refs(aggregate, &unsupported_claim.checked_block_refs);
        }
    }

    aggregates
        .into_values()
        .map(|aggregate| SessionSynthesisClaim {
            version: CONTRACT_VERSION.to_string(),
            claim_id: aggregate.claim_id,
            statement: aggregate.statement,
            status: aggregate.status,
            snapshot_ids: aggregate.snapshot_ids.into_iter().collect(),
            support_refs: aggregate.support_refs.into_iter().collect(),
            citations: aggregate.citations,
        })
        .collect()
}

fn ensure_aggregate<'a>(
    aggregates: &'a mut BTreeMap<String, AggregateClaim>,
    claim_id: &str,
    statement: &str,
    default_status: SessionSynthesisClaimStatus,
) -> &'a mut AggregateClaim {
    let key = claim_id.to_string();
    let aggregate = aggregates.entry(key).or_insert_with(|| AggregateClaim {
        claim_id: claim_id.to_string(),
        statement: statement.to_string(),
        status: default_status,
        snapshot_ids: BTreeSet::new(),
        support_refs: BTreeSet::new(),
        citations: Vec::new(),
        citation_keys: BTreeSet::new(),
    });
    if aggregate.statement.trim().is_empty() {
        aggregate.statement = statement.to_string();
    }
    aggregate
}

fn update_claim_status(aggregate: &mut AggregateClaim, next_status: SessionSynthesisClaimStatus) {
    if claim_status_priority(next_status.clone()) > claim_status_priority(aggregate.status.clone())
    {
        aggregate.status = next_status;
    }
}

fn record_snapshot_id(aggregate: &mut AggregateClaim, snapshot_id: &str) {
    aggregate.snapshot_ids.insert(snapshot_id.to_string());
}

fn record_support_refs(aggregate: &mut AggregateClaim, support_refs: &[String]) {
    aggregate.support_refs.extend(support_refs.iter().cloned());
}

fn record_citation(aggregate: &mut AggregateClaim, citation: &EvidenceCitation) {
    let citation_key = format!(
        "{}|{}|{:?}|{:?}",
        citation.url, citation.retrieved_at, citation.source_type, citation.source_risk
    );
    if aggregate.citation_keys.insert(citation_key) {
        aggregate.citations.push(citation.clone());
    }
}

fn claim_status_priority(status: SessionSynthesisClaimStatus) -> usize {
    match status {
        SessionSynthesisClaimStatus::InsufficientEvidence => 0,
        SessionSynthesisClaimStatus::NeedsMoreBrowsing => 1,
        SessionSynthesisClaimStatus::EvidenceSupported => 2,
        SessionSynthesisClaimStatus::Contradicted => 3,
    }
}
