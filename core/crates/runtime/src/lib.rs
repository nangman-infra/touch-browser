use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, Value};
use thiserror::Error;
use touch_browser_acquisition::AcquisitionEngine;
use touch_browser_contracts::{
    ActionCommand, ActionName, PolicyProfile, ReplayTranscript, RiskClass, SessionMode,
    SessionState, SessionStatus, SessionSynthesisClaimStatus, SessionSynthesisReport,
    SnapshotDocument, SourceRisk, SourceType, TranscriptKind, TranscriptPayloadType,
    CONTRACT_VERSION,
};
use touch_browser_evidence::{ClaimRequest, EvidenceExtractor, EvidenceInput};
use touch_browser_memory::{
    compact_working_set, diff_snapshots, plan_memory_turn, summarize_turns, CompactionResult,
    SnapshotDiff,
};
use touch_browser_observation::{
    recommend_requested_tokens, ObservationCompiler, ObservationInput,
};

mod session_claims;
mod session_journal;

use session_claims::aggregate_session_claims;
use session_journal::{append_snapshot, record_command, record_observation, record_system};

pub fn runtime_banner() -> &'static str {
    "touch-browser-runtime ready"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogDocument {
    pub source_url: String,
    pub html: String,
    pub source_type: SourceType,
    pub source_risk: SourceRisk,
    pub source_label: Option<String>,
    pub aliases: Vec<String>,
}

impl CatalogDocument {
    pub fn new(
        source_url: impl Into<String>,
        html: impl Into<String>,
        source_type: SourceType,
        source_risk: SourceRisk,
        source_label: Option<String>,
    ) -> Self {
        Self {
            source_url: source_url.into(),
            html: html.into(),
            source_type,
            source_risk,
            source_label,
            aliases: Vec::new(),
        }
    }

    pub fn with_aliases(mut self, aliases: Vec<String>) -> Self {
        self.aliases = aliases;
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct FixtureCatalog {
    documents: BTreeMap<String, CatalogDocument>,
    aliases: BTreeMap<String, String>,
}

impl FixtureCatalog {
    pub fn register(&mut self, document: CatalogDocument) {
        let source_url = document.source_url.clone();

        for alias in &document.aliases {
            self.aliases.insert(alias.clone(), source_url.clone());
        }

        self.documents.insert(source_url, document);
    }

    pub fn get(&self, source_url: &str) -> Option<&CatalogDocument> {
        self.documents.get(source_url)
    }

    pub fn resolve_link(&self, current_url: &str, href: &str) -> Option<&CatalogDocument> {
        if let Some(document) = self.documents.get(href) {
            return Some(document);
        }

        if let Some(source_url) = self.aliases.get(href) {
            return self.documents.get(source_url);
        }

        if href.starts_with('#') {
            return self.documents.get(current_url);
        }

        if href.starts_with('/') {
            let normalized = href.trim_start_matches('/').to_ascii_lowercase();

            if let Some((_, document)) = self.documents.iter().find(|(source_url, _)| {
                source_url
                    .rsplit('/')
                    .next()
                    .map(|slug| slug.starts_with(&normalized))
                    .unwrap_or(false)
            }) {
                return Some(document);
            }
        }

        None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSnapshotRecord {
    pub snapshot_id: String,
    pub snapshot: SnapshotDocument,
    pub source_risk: SourceRisk,
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRecord {
    pub snapshot_id: String,
    pub report: touch_browser_contracts::EvidenceReport,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReadOnlySession {
    pub state: SessionState,
    pub transcript: ReplayTranscript,
    pub snapshots: Vec<SessionSnapshotRecord>,
    pub evidence_reports: Vec<EvidenceRecord>,
}

impl ReadOnlySession {
    fn current_snapshot(&self) -> Option<&SessionSnapshotRecord> {
        self.snapshots.last()
    }

    pub fn current_snapshot_record(&self) -> Option<&SessionSnapshotRecord> {
        self.current_snapshot()
    }

    fn current_evidence(&self) -> Option<&touch_browser_contracts::EvidenceReport> {
        self.evidence_reports.last().map(|record| &record.report)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClaimInput {
    pub id: String,
    pub statement: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffInput {
    pub from_snapshot_id: String,
    pub to_snapshot_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompactInput {
    pub limit: usize,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ReadOnlyRuntime {
    observation: ObservationCompiler,
    evidence: EvidenceExtractor,
}

impl ReadOnlyRuntime {
    pub fn start_session(
        &self,
        session_id: impl Into<String>,
        opened_at: impl Into<String>,
    ) -> ReadOnlySession {
        let session_id = session_id.into();
        let opened_at = opened_at.into();

        ReadOnlySession {
            state: SessionState {
                version: CONTRACT_VERSION.to_string(),
                session_id: session_id.clone(),
                mode: SessionMode::ReadOnly,
                status: SessionStatus::Idle,
                policy_profile: PolicyProfile::ResearchReadOnly,
                current_url: None,
                opened_at: opened_at.clone(),
                updated_at: opened_at,
                visited_urls: Vec::new(),
                snapshot_ids: Vec::new(),
                working_set_refs: Vec::new(),
            },
            transcript: ReplayTranscript {
                version: CONTRACT_VERSION.to_string(),
                session_id,
                entries: Vec::new(),
            },
            snapshots: Vec::new(),
            evidence_reports: Vec::new(),
        }
    }

    pub fn open(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        target_url: &str,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(target_url.to_string()),
                risk_class: RiskClass::Low,
                reason: "Open a read-only research document.".to_string(),
                input: None,
            },
        )?;

        let (snapshot, source_risk, source_label) = self.load_snapshot(catalog, target_url)?;
        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn open_live(
        &self,
        session: &mut ReadOnlySession,
        acquisition: &mut AcquisitionEngine,
        target_url: &str,
        requested_tokens: usize,
        source_risk: SourceRisk,
        source_label: Option<String>,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(target_url.to_string()),
                risk_class: RiskClass::Low,
                reason: "Open a live read-only research document.".to_string(),
                input: None,
            },
        )?;

        let acquired = acquisition
            .fetch(target_url)
            .map_err(RuntimeError::Acquisition)?;
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::AcquisitionRecord,
            to_value(&acquired.record)?,
        )?;

        let effective_budget = recommend_requested_tokens(&acquired.body, requested_tokens);
        let snapshot = self
            .observation
            .compile(&ObservationInput::new(
                acquired.record.final_url.clone(),
                acquired.record.source_type.clone(),
                acquired.body,
                effective_budget,
            ))
            .map_err(RuntimeError::Observation)?;

        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

    pub fn open_snapshot(
        &self,
        session: &mut ReadOnlySession,
        target_url: &str,
        snapshot: SnapshotDocument,
        source_risk: SourceRisk,
        source_label: Option<String>,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(target_url.to_string()),
                risk_class: RiskClass::Low,
                reason: "Open a browser-backed read-only research document.".to_string(),
                input: None,
            },
        )?;

        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

    pub fn read(
        &self,
        session: &mut ReadOnlySession,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Read,
                target_ref: None,
                target_url: session.state.current_url.clone(),
                risk_class: RiskClass::Low,
                reason: "Read the current semantic snapshot.".to_string(),
                input: None,
            },
        )?;

        let snapshot = session
            .current_snapshot()
            .ok_or(RuntimeError::NoCurrentSnapshot)?
            .snapshot
            .clone();
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SnapshotDocument,
            to_value(&snapshot)?,
        )?;
        record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SessionState,
            to_value(&session.state)?,
        )?;
        Ok(snapshot)
    }

    pub fn follow(
        &self,
        session: &mut ReadOnlySession,
        catalog: &FixtureCatalog,
        target_ref: &str,
        timestamp: &str,
    ) -> Result<SnapshotDocument, RuntimeError> {
        record_command(
            session,
            timestamp,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Follow,
                target_ref: Some(target_ref.to_string()),
                target_url: None,
                risk_class: RiskClass::Low,
                reason: "Follow a link from the current snapshot.".to_string(),
                input: None,
            },
        )?;

        let current_snapshot = session
            .current_snapshot()
            .ok_or(RuntimeError::NoCurrentSnapshot)?;
        let href = current_snapshot
            .snapshot
            .blocks
            .iter()
            .find(|block| block.stable_ref == target_ref)
            .and_then(|block| block.attributes.get("href"))
            .and_then(Value::as_str)
            .ok_or_else(|| RuntimeError::MissingHref(target_ref.to_string()))?;
        let current_url = session
            .state
            .current_url
            .clone()
            .ok_or(RuntimeError::MissingCurrentUrl)?;
        let resolved = catalog
            .resolve_link(&current_url, href)
            .ok_or_else(|| RuntimeError::UnresolvedLink(href.to_string()))?;
        let (snapshot, source_risk, source_label) =
            self.load_snapshot(catalog, &resolved.source_url)?;
        append_snapshot(
            session,
            timestamp,
            snapshot.clone(),
            source_risk,
            source_label,
        )?;
        Ok(snapshot)
    }

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

    pub fn replay(
        &self,
        catalog: &FixtureCatalog,
        transcript: &ReplayTranscript,
        opened_at: &str,
    ) -> Result<ReadOnlySession, RuntimeError> {
        let mut session = self.start_session(transcript.session_id.clone(), opened_at.to_string());

        for entry in &transcript.entries {
            if entry.kind != TranscriptKind::Command
                || entry.payload_type != TranscriptPayloadType::ActionCommand
            {
                continue;
            }

            let command: ActionCommand = serde_json::from_value(entry.payload.clone())?;

            match command.action {
                ActionName::Open => {
                    let target_url = command
                        .target_url
                        .as_deref()
                        .ok_or(RuntimeError::ReplayMissingTarget)?;
                    self.open(&mut session, catalog, target_url, &entry.timestamp)?;
                }
                ActionName::Read => {
                    let _ = self.read(&mut session, &entry.timestamp)?;
                }
                ActionName::Follow => {
                    let target_ref = command
                        .target_ref
                        .as_deref()
                        .ok_or(RuntimeError::ReplayMissingTarget)?;
                    let _ = self.follow(&mut session, catalog, target_ref, &entry.timestamp)?;
                }
                ActionName::Extract => {
                    let claims = parse_claim_inputs(command.input.as_ref())?;
                    let _ = self.extract(&mut session, claims, &entry.timestamp)?;
                }
                ActionName::Diff => {
                    let diff_input: DiffInput = serde_json::from_value(
                        command.input.ok_or(RuntimeError::ReplayMissingInput)?,
                    )?;
                    let _ = self.diff(&mut session, diff_input, &entry.timestamp)?;
                }
                ActionName::Compact => {
                    let compact_input: CompactInput = serde_json::from_value(
                        command.input.ok_or(RuntimeError::ReplayMissingInput)?,
                    )?;
                    let _ = self.compact(&mut session, compact_input, &entry.timestamp)?;
                }
                _ => {}
            }
        }

        Ok(session)
    }

    fn load_snapshot(
        &self,
        catalog: &FixtureCatalog,
        target_url: &str,
    ) -> Result<(SnapshotDocument, SourceRisk, Option<String>), RuntimeError> {
        let document = catalog
            .get(target_url)
            .ok_or_else(|| RuntimeError::UnknownSource(target_url.to_string()))?;

        let snapshot = self
            .observation
            .compile(&ObservationInput::new(
                document.source_url.clone(),
                document.source_type.clone(),
                document.html.clone(),
                512,
            ))
            .map_err(RuntimeError::Observation)?;

        Ok((
            snapshot,
            document.source_risk.clone(),
            document.source_label.clone(),
        ))
    }
}

fn parse_claim_inputs(input: Option<&Value>) -> Result<Vec<ClaimInput>, RuntimeError> {
    let claims_value = input
        .and_then(|value| value.get("claims"))
        .cloned()
        .ok_or(RuntimeError::ReplayMissingInput)?;
    serde_json::from_value(claims_value).map_err(RuntimeError::Serde)
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("unknown source url: {0}")]
    UnknownSource(String),
    #[error("no current snapshot is available")]
    NoCurrentSnapshot,
    #[error("missing current url")]
    MissingCurrentUrl,
    #[error("follow target ref has no href: {0}")]
    MissingHref(String),
    #[error("could not resolve link target: {0}")]
    UnresolvedLink(String),
    #[error("missing snapshot id: {0}")]
    MissingSnapshotId(String),
    #[error("replay command is missing a target")]
    ReplayMissingTarget,
    #[error("replay command is missing input")]
    ReplayMissingInput,
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("acquisition error: {0}")]
    Acquisition(#[from] touch_browser_acquisition::AcquisitionError),
    #[error("observation error: {0}")]
    Observation(#[from] touch_browser_observation::ObservationError),
    #[error("evidence error: {0}")]
    Evidence(#[from] touch_browser_evidence::EvidenceError),
}

#[cfg(test)]
mod tests;
