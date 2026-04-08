use std::collections::BTreeMap;

use touch_browser_contracts::{
    SessionSynthesisClaimStatus, SessionSynthesisReport, CONTRACT_VERSION,
};
use touch_browser_memory::{plan_memory_turn, summarize_turns};

use crate::{aggregate_session_claims, ReadOnlyRuntime, ReadOnlySession, RuntimeError};

impl ReadOnlyRuntime {
    pub fn synthesize_session(
        &self,
        session: &ReadOnlySession,
        timestamp: &str,
        note_limit: usize,
    ) -> Result<SessionSynthesisReport, RuntimeError> {
        let mut memory_refs = Vec::new();
        let mut memory_turns = Vec::new();
        let evidence_by_snapshot =
            session
                .evidence_reports
                .iter()
                .fold(BTreeMap::new(), |mut grouped, record| {
                    grouped.insert(record.snapshot_id.clone(), &record.report);
                    grouped
                });

        for (turn_index, snapshot_record) in session.snapshots.iter().enumerate() {
            let evidence = evidence_by_snapshot
                .get(&snapshot_record.snapshot_id)
                .copied();
            let turn = plan_memory_turn(
                turn_index + 1,
                &snapshot_record.snapshot_id,
                &snapshot_record.snapshot,
                evidence,
                &memory_refs,
                6,
            );
            memory_refs = turn.kept_refs.clone();
            memory_turns.push(turn);
        }

        let summary = summarize_turns(&memory_turns, note_limit);
        let aggregated_claims = aggregate_session_claims(session);

        Ok(SessionSynthesisReport {
            version: CONTRACT_VERSION.to_string(),
            session_id: session.state.session_id.clone(),
            generated_at: timestamp.to_string(),
            snapshot_count: session.snapshots.len(),
            evidence_report_count: session.evidence_reports.len(),
            visited_urls: session.state.visited_urls.clone(),
            working_set_refs: session.state.working_set_refs.clone(),
            synthesized_notes: summary.synthesized_notes,
            supported_claims: aggregated_claims
                .iter()
                .filter(|claim| claim.status == SessionSynthesisClaimStatus::EvidenceSupported)
                .cloned()
                .collect(),
            contradicted_claims: aggregated_claims
                .iter()
                .filter(|claim| claim.status == SessionSynthesisClaimStatus::Contradicted)
                .cloned()
                .collect(),
            unsupported_claims: aggregated_claims
                .iter()
                .filter(|claim| claim.status == SessionSynthesisClaimStatus::InsufficientEvidence)
                .cloned()
                .collect(),
            needs_more_browsing_claims: aggregated_claims
                .iter()
                .filter(|claim| claim.status == SessionSynthesisClaimStatus::NeedsMoreBrowsing)
                .cloned()
                .collect(),
        })
    }
}
