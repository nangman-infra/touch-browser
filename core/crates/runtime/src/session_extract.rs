use serde_json::{json, to_value};
use touch_browser_contracts::{
    ActionCommand, ActionName, RiskClass, TranscriptPayloadType, CONTRACT_VERSION,
};
use touch_browser_memory::{compact_working_set, diff_snapshots, CompactionResult, SnapshotDiff};

use crate::{
    session_journal::{record_command, record_observation, record_system},
    ClaimInput, CompactInput, DiffInput, EvidenceRecord, ReadOnlyRuntime, ReadOnlySession,
    RuntimeError,
};
use touch_browser_evidence::{ClaimRequest, EvidenceInput};

impl ReadOnlyRuntime {
    pub fn extract(
        &self,
        session: &mut ReadOnlySession,
        claims: Vec<ClaimInput>,
        timestamp: &str,
    ) -> Result<touch_browser_contracts::EvidenceReport, RuntimeError> {
        let input_claims = claims
            .iter()
            .map(|claim| {
                json!({
                    "id": claim.id,
                    "statement": claim.statement
                })
            })
            .collect::<Vec<_>>();
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Extract,
                target_ref: None,
                target_url: session.state.current_url.clone(),
                risk_class: RiskClass::Low,
                reason: "Extract supported and unsupported claims.".to_string(),
                input: Some(json!({ "claims": input_claims })),
            },
        )?;

        let current_snapshot = session
            .current_snapshot()
            .cloned()
            .ok_or(RuntimeError::NoCurrentSnapshot)?;
        let source_risk = current_snapshot.source_risk.clone();
        let source_label = current_snapshot.source_label.clone();
        let report = self
            .evidence
            .extract(&EvidenceInput::new(
                current_snapshot.snapshot.clone(),
                claims
                    .into_iter()
                    .map(|claim| ClaimRequest::new(claim.id, claim.statement))
                    .collect(),
                timestamp,
                source_risk,
                source_label,
            ))
            .map_err(RuntimeError::Evidence)?;

        session.evidence_reports.push(EvidenceRecord {
            snapshot_id: current_snapshot.snapshot_id.clone(),
            report: report.clone(),
        });
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::EvidenceReport,
            to_value(&report)?,
        )?;

        let compacted = compact_working_set(
            &current_snapshot.snapshot,
            Some(&report),
            &session.state.working_set_refs,
            6,
        );
        session.state.working_set_refs = compacted.kept_refs;
        session.state.updated_at = timestamp.to_string();
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SessionState,
            to_value(&session.state)?,
        )?;

        Ok(report)
    }

    pub fn diff(
        &self,
        session: &mut ReadOnlySession,
        input: DiffInput,
        timestamp: &str,
    ) -> Result<SnapshotDiff, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Diff,
                target_ref: None,
                target_url: None,
                risk_class: RiskClass::Low,
                reason: "Compare two snapshots in the current session.".to_string(),
                input: Some(to_value(&input)?),
            },
        )?;

        let from_snapshot = session
            .snapshots
            .iter()
            .find(|record| record.snapshot_id == input.from_snapshot_id)
            .ok_or_else(|| RuntimeError::MissingSnapshotId(input.from_snapshot_id.clone()))?;
        let to_snapshot = session
            .snapshots
            .iter()
            .find(|record| record.snapshot_id == input.to_snapshot_id)
            .ok_or_else(|| RuntimeError::MissingSnapshotId(input.to_snapshot_id.clone()))?;

        let diff = diff_snapshots(
            &from_snapshot.snapshot_id,
            &from_snapshot.snapshot,
            &to_snapshot.snapshot_id,
            &to_snapshot.snapshot,
        );
        record_system(session, timestamp, json!({ "diff": diff }))?;
        Ok(diff)
    }

    pub fn compact(
        &self,
        session: &mut ReadOnlySession,
        input: CompactInput,
        timestamp: &str,
    ) -> Result<CompactionResult, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Compact,
                target_ref: None,
                target_url: session.state.current_url.clone(),
                risk_class: RiskClass::Low,
                reason: "Compact the working set for the current session.".to_string(),
                input: Some(to_value(&input)?),
            },
        )?;

        let snapshot = session
            .current_snapshot()
            .ok_or(RuntimeError::NoCurrentSnapshot)?;
        let compacted = compact_working_set(
            &snapshot.snapshot,
            session.current_evidence(),
            &session.state.working_set_refs,
            input.limit,
        );
        session.state.working_set_refs = compacted.kept_refs.clone();
        session.state.updated_at = timestamp.to_string();
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SessionState,
            to_value(&session.state)?,
        )?;
        Ok(compacted)
    }
}
