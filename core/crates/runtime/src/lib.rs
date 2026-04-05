use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::{json, to_value, Value};
use thiserror::Error;
use touch_browser_acquisition::AcquisitionEngine;
use touch_browser_contracts::{
    ActionCommand, ActionName, PolicyProfile, ReplayTranscript, ReplayTranscriptEntry, RiskClass,
    SessionMode, SessionState, SessionStatus, SessionSynthesisClaim, SessionSynthesisClaimStatus,
    SessionSynthesisReport, SnapshotDocument, SourceRisk, SourceType, TranscriptKind,
    TranscriptPayloadType, CONTRACT_VERSION,
};
use touch_browser_evidence::{ClaimRequest, EvidenceExtractor, EvidenceInput};
use touch_browser_memory::{
    compact_working_set, diff_snapshots, plan_memory_turn, summarize_turns, CompactionResult,
    SnapshotDiff,
};
use touch_browser_observation::{
    recommend_requested_tokens, ObservationCompiler, ObservationInput,
};

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
        self.record_command(
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
        self.append_snapshot(
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
        self.record_command(
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
        self.record_observation(
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

        self.append_snapshot(
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
        self.record_command(
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

        self.append_snapshot(
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
        self.record_command(
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
        self.record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SnapshotDocument,
            to_value(&snapshot)?,
        )?;
        self.record_observation(
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
        self.record_command(
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
        self.append_snapshot(
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
        self.record_command(
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
        self.record_observation(
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
        self.record_observation(
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
        self.record_command(
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
        self.record_system(session, timestamp, json!({ "diff": diff }))?;
        Ok(diff)
    }

    pub fn compact(
        &self,
        session: &mut ReadOnlySession,
        input: CompactInput,
        timestamp: &str,
    ) -> Result<CompactionResult, RuntimeError> {
        self.record_command(
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
        self.record_observation(
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
                .filter(|claim| {
                    claim.status == SessionSynthesisClaimStatus::InsufficientEvidence
                })
                .cloned()
                .collect(),
            needs_more_browsing_claims: aggregated_claims
                .iter()
                .filter(|claim| {
                    claim.status == SessionSynthesisClaimStatus::NeedsMoreBrowsing
                })
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

    fn append_snapshot(
        &self,
        session: &mut ReadOnlySession,
        timestamp: &str,
        snapshot: SnapshotDocument,
        source_risk: SourceRisk,
        source_label: Option<String>,
    ) -> Result<(), RuntimeError> {
        let snapshot_id = format!(
            "snap_{}_{}",
            session
                .state
                .session_id
                .strip_prefix('s')
                .unwrap_or(session.state.session_id.as_str()),
            session.snapshots.len() + 1
        );
        session.snapshots.push(SessionSnapshotRecord {
            snapshot_id: snapshot_id.clone(),
            snapshot: snapshot.clone(),
            source_risk,
            source_label,
        });
        session.state.status = SessionStatus::Active;
        session.state.current_url = Some(snapshot.source.source_url.clone());
        session.state.updated_at = timestamp.to_string();
        if !session
            .state
            .visited_urls
            .iter()
            .any(|url| url == &snapshot.source.source_url)
        {
            session
                .state
                .visited_urls
                .push(snapshot.source.source_url.clone());
        }
        session.state.snapshot_ids.push(snapshot_id);
        session.state.working_set_refs =
            compact_working_set(&snapshot, None, &session.state.working_set_refs, 6).kept_refs;
        self.record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SnapshotDocument,
            to_value(&snapshot)?,
        )?;
        self.record_observation(
            session,
            timestamp,
            TranscriptPayloadType::SessionState,
            to_value(&session.state)?,
        )?;
        Ok(())
    }

    fn record_command(
        &self,
        session: &mut ReadOnlySession,
        timestamp: &str,
        command: ActionCommand,
    ) -> Result<(), RuntimeError> {
        let payload = to_value(command)?;
        let seq = session.transcript.entries.len() + 1;
        session.transcript.entries.push(ReplayTranscriptEntry {
            seq,
            timestamp: timestamp.to_string(),
            kind: TranscriptKind::Command,
            payload_type: TranscriptPayloadType::ActionCommand,
            payload,
        });
        Ok(())
    }

    fn record_observation(
        &self,
        session: &mut ReadOnlySession,
        timestamp: &str,
        payload_type: TranscriptPayloadType,
        payload: Value,
    ) -> Result<(), RuntimeError> {
        let seq = session.transcript.entries.len() + 1;
        session.transcript.entries.push(ReplayTranscriptEntry {
            seq,
            timestamp: timestamp.to_string(),
            kind: TranscriptKind::Observation,
            payload_type,
            payload,
        });
        Ok(())
    }

    fn record_system(
        &self,
        session: &mut ReadOnlySession,
        timestamp: &str,
        payload: Value,
    ) -> Result<(), RuntimeError> {
        let seq = session.transcript.entries.len() + 1;
        session.transcript.entries.push(ReplayTranscriptEntry {
            seq,
            timestamp: timestamp.to_string(),
            kind: TranscriptKind::System,
            payload_type: TranscriptPayloadType::JsonRpc,
            payload,
        });
        Ok(())
    }
}

fn parse_claim_inputs(input: Option<&Value>) -> Result<Vec<ClaimInput>, RuntimeError> {
    let claims_value = input
        .and_then(|value| value.get("claims"))
        .cloned()
        .ok_or(RuntimeError::ReplayMissingInput)?;
    serde_json::from_value(claims_value).map_err(RuntimeError::Serde)
}

fn aggregate_session_claims(session: &ReadOnlySession) -> Vec<SessionSynthesisClaim> {
    #[derive(Debug, Clone)]
    struct Aggregate {
        claim_id: String,
        statement: String,
        status: SessionSynthesisClaimStatus,
        snapshot_ids: BTreeSet<String>,
        support_refs: BTreeSet<String>,
        citations: Vec<touch_browser_contracts::EvidenceCitation>,
        citation_keys: BTreeSet<String>,
    }

    let mut aggregates = BTreeMap::<(String, String), Aggregate>::new();

    for evidence_record in &session.evidence_reports {
        for supported_claim in &evidence_record.report.supported_claims {
            let key = (
                supported_claim.claim_id.clone(),
                supported_claim.statement.clone(),
            );
            let aggregate = aggregates.entry(key).or_insert_with(|| Aggregate {
                claim_id: supported_claim.claim_id.clone(),
                statement: supported_claim.statement.clone(),
                status: SessionSynthesisClaimStatus::EvidenceSupported,
                snapshot_ids: BTreeSet::new(),
                support_refs: BTreeSet::new(),
                citations: Vec::new(),
                citation_keys: BTreeSet::new(),
            });

            aggregate.status = SessionSynthesisClaimStatus::EvidenceSupported;
            aggregate
                .snapshot_ids
                .insert(evidence_record.snapshot_id.clone());
            for support_ref in &supported_claim.support {
                aggregate.support_refs.insert(support_ref.clone());
            }
            let citation_key = format!(
                "{}|{}|{:?}|{:?}",
                supported_claim.citation.url,
                supported_claim.citation.retrieved_at,
                supported_claim.citation.source_type,
                supported_claim.citation.source_risk
            );
            if aggregate.citation_keys.insert(citation_key) {
                aggregate.citations.push(supported_claim.citation.clone());
            }
        }

        for unsupported_claim in &evidence_record.report.contradicted_claims {
            let key = (
                unsupported_claim.claim_id.clone(),
                unsupported_claim.statement.clone(),
            );
            let aggregate = aggregates.entry(key).or_insert_with(|| Aggregate {
                claim_id: unsupported_claim.claim_id.clone(),
                statement: unsupported_claim.statement.clone(),
                status: SessionSynthesisClaimStatus::Contradicted,
                snapshot_ids: BTreeSet::new(),
                support_refs: BTreeSet::new(),
                citations: Vec::new(),
                citation_keys: BTreeSet::new(),
            });

            if claim_status_priority(SessionSynthesisClaimStatus::Contradicted)
                > claim_status_priority(aggregate.status.clone())
            {
                aggregate.status = SessionSynthesisClaimStatus::Contradicted;
            }
            aggregate
                .snapshot_ids
                .insert(evidence_record.snapshot_id.clone());
            for checked_ref in &unsupported_claim.checked_block_refs {
                aggregate.support_refs.insert(checked_ref.clone());
            }
        }

        for unsupported_claim in &evidence_record.report.unsupported_claims {
            let key = (
                unsupported_claim.claim_id.clone(),
                unsupported_claim.statement.clone(),
            );
            let aggregate = aggregates.entry(key).or_insert_with(|| Aggregate {
                claim_id: unsupported_claim.claim_id.clone(),
                statement: unsupported_claim.statement.clone(),
                status: SessionSynthesisClaimStatus::InsufficientEvidence,
                snapshot_ids: BTreeSet::new(),
                support_refs: BTreeSet::new(),
                citations: Vec::new(),
                citation_keys: BTreeSet::new(),
            });

            if claim_status_priority(SessionSynthesisClaimStatus::InsufficientEvidence)
                > claim_status_priority(aggregate.status.clone())
            {
                aggregate.status = SessionSynthesisClaimStatus::InsufficientEvidence;
            }
            aggregate
                .snapshot_ids
                .insert(evidence_record.snapshot_id.clone());
            for checked_ref in &unsupported_claim.checked_block_refs {
                aggregate.support_refs.insert(checked_ref.clone());
            }
        }

        for unsupported_claim in &evidence_record.report.needs_more_browsing_claims {
            let key = (
                unsupported_claim.claim_id.clone(),
                unsupported_claim.statement.clone(),
            );
            let aggregate = aggregates.entry(key).or_insert_with(|| Aggregate {
                claim_id: unsupported_claim.claim_id.clone(),
                statement: unsupported_claim.statement.clone(),
                status: SessionSynthesisClaimStatus::NeedsMoreBrowsing,
                snapshot_ids: BTreeSet::new(),
                support_refs: BTreeSet::new(),
                citations: Vec::new(),
                citation_keys: BTreeSet::new(),
            });

            if claim_status_priority(SessionSynthesisClaimStatus::NeedsMoreBrowsing)
                > claim_status_priority(aggregate.status.clone())
            {
                aggregate.status = SessionSynthesisClaimStatus::NeedsMoreBrowsing;
            }
            aggregate
                .snapshot_ids
                .insert(evidence_record.snapshot_id.clone());
            for checked_ref in &unsupported_claim.checked_block_refs {
                aggregate.support_refs.insert(checked_ref.clone());
            }
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

fn claim_status_priority(status: SessionSynthesisClaimStatus) -> usize {
    match status {
        SessionSynthesisClaimStatus::InsufficientEvidence => 0,
        SessionSynthesisClaimStatus::NeedsMoreBrowsing => 1,
        SessionSynthesisClaimStatus::Contradicted => 2,
        SessionSynthesisClaimStatus::EvidenceSupported => 3,
    }
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
mod tests {
    use std::{
        fs,
        io::Cursor,
        net::TcpListener,
        path::PathBuf,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        thread,
    };

    use serde::Deserialize;
    use tiny_http::{Header, Response as TinyResponse, Server, StatusCode};
    use touch_browser_acquisition::{AcquisitionConfig, AcquisitionEngine};
    use touch_browser_contracts::{
        ReplayTranscript, SessionMode, SessionStatus, SourceRisk, SourceType, TranscriptKind,
        TranscriptPayloadType,
    };
    use touch_browser_memory::{plan_memory_turn, summarize_turns};

    use super::{
        CatalogDocument, ClaimInput, CompactInput, DiffInput, FixtureCatalog, ReadOnlyRuntime,
    };

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureMetadata {
        title: String,
        source_uri: String,
        html_path: String,
        risk: String,
    }

    #[test]
    fn executes_read_only_fixture_session_and_replays_deterministically() {
        let runtime = ReadOnlyRuntime::default();
        let catalog = fixture_catalog();
        let mut session = runtime.start_session("sfixture001", "2026-03-14T00:00:00+09:00");

        let opened = runtime
            .open(
                &mut session,
                &catalog,
                "fixture://research/static-docs/getting-started",
                "2026-03-14T00:00:01+09:00",
            )
            .expect("open should work");
        assert_eq!(
            opened.source.source_url,
            "fixture://research/static-docs/getting-started"
        );
        assert_eq!(session.state.mode, SessionMode::ReadOnly);
        assert_eq!(session.state.status, SessionStatus::Active);

        let _ = runtime
            .read(&mut session, "2026-03-14T00:00:02+09:00")
            .expect("read should work");
        let followed = runtime
            .follow(
                &mut session,
                &catalog,
                "rnav:link:pricing",
                "2026-03-14T00:00:03+09:00",
            )
            .expect("follow should work");
        assert_eq!(
            followed.source.source_url,
            "fixture://research/citation-heavy/pricing"
        );

        let report = runtime
            .extract(
                &mut session,
                vec![
                    ClaimInput {
                        id: "c1".to_string(),
                        statement: "The Starter plan costs $29 per month.".to_string(),
                    },
                    ClaimInput {
                        id: "c2".to_string(),
                        statement: "There is an Enterprise plan.".to_string(),
                    },
                ],
                "2026-03-14T00:00:04+09:00",
            )
            .expect("extract should work");
        assert_eq!(report.supported_claims.len(), 1);
        assert_eq!(report.unsupported_claims.len(), 1);

        let diff = runtime
            .diff(
                &mut session,
                DiffInput {
                    from_snapshot_id: "snap_fixture001_1".to_string(),
                    to_snapshot_id: "snap_fixture001_2".to_string(),
                },
                "2026-03-14T00:00:05+09:00",
            )
            .expect("diff should work");
        assert!(diff
            .added_refs
            .contains(&"rmain:table:plan-monthly-price-snapshots-starter-29-10-000-t".to_string()));

        let compacted = runtime
            .compact(
                &mut session,
                CompactInput { limit: 3 },
                "2026-03-14T00:00:06+09:00",
            )
            .expect("compact should work");
        assert_eq!(compacted.kept_refs.len(), 3);

        let transcript_json =
            serde_json::to_string_pretty(&session.transcript).expect("transcript serialize");
        let replay_transcript: ReplayTranscript =
            serde_json::from_str(&transcript_json).expect("transcript deserialize");
        let replayed = runtime
            .replay(&catalog, &replay_transcript, "2026-03-14T00:00:00+09:00")
            .expect("replay should work");

        assert_eq!(session.state, replayed.state);
        assert_eq!(session.snapshots, replayed.snapshots);
        assert_eq!(session.evidence_reports, replayed.evidence_reports);

        let action_entries = session
            .transcript
            .entries
            .iter()
            .filter(|entry| entry.kind == TranscriptKind::Command)
            .collect::<Vec<_>>();
        assert_eq!(action_entries.len(), 6);
        assert!(session
            .transcript
            .entries
            .iter()
            .any(|entry| entry.payload_type == TranscriptPayloadType::EvidenceReport));
    }

    #[test]
    fn opens_live_documents_via_acquisition_and_records_fetch_metadata() {
        let runtime = ReadOnlyRuntime::default();
        let mut acquisition =
            AcquisitionEngine::new(AcquisitionConfig::default()).expect("acquisition");
        let requests = Arc::new(AtomicUsize::new(0));
        let server = LiveTestServer::start(requests.clone());
        let mut session = runtime.start_session("slive001", "2026-03-14T00:00:00+09:00");

        let snapshot = runtime
            .open_live(
                &mut session,
                &mut acquisition,
                &format!("{}/start#intro", server.base_url()),
                512,
                SourceRisk::Medium,
                Some("live docs".to_string()),
                "2026-03-14T00:00:01+09:00",
            )
            .expect("live open should work");
        assert_eq!(
            snapshot.source.source_url,
            format!("{}/docs", server.base_url())
        );
        assert_eq!(
            session.state.current_url,
            Some(format!("{}/docs", server.base_url()))
        );
        assert!(session
            .transcript
            .entries
            .iter()
            .any(|entry| entry.payload_type == TranscriptPayloadType::AcquisitionRecord));

        let report = runtime
            .extract(
                &mut session,
                vec![ClaimInput {
                    id: "live-1".to_string(),
                    statement: "The Starter plan costs $29 per month.".to_string(),
                }],
                "2026-03-14T00:00:02+09:00",
            )
            .expect("extract should work on live snapshot");
        assert_eq!(report.supported_claims.len(), 1);
        assert_eq!(requests.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn maintains_bounded_memory_across_twenty_actions() {
        let runtime = ReadOnlyRuntime::default();
        let catalog = fixture_catalog();
        let mut session = runtime.start_session("smemory001", "2026-03-14T00:00:00+09:00");
        let sequence = [
            (
                "fixture://research/static-docs/getting-started",
                "docs-1",
                "Touch Browser compiles web pages into semantic state for research agents.",
            ),
            (
                "fixture://research/citation-heavy/pricing",
                "pricing-1",
                "The Starter plan costs $29 per month.",
            ),
            (
                "fixture://research/navigation/api-reference",
                "api-1",
                "Snapshot responses include stable refs and evidence metadata.",
            ),
        ];

        let mut memory_refs = Vec::new();
        let mut memory_turns = Vec::new();

        for action_index in 0..10 {
            let step = sequence[action_index % sequence.len()];
            let open_timestamp = format!("2026-03-14T00:{:02}:00+09:00", action_index * 2);
            let extract_timestamp = format!("2026-03-14T00:{:02}:30+09:00", action_index * 2 + 1);

            runtime
                .open(&mut session, &catalog, step.0, &open_timestamp)
                .expect("open should work");
            let snapshot_record = session.snapshots.last().expect("snapshot after open");
            let open_turn = plan_memory_turn(
                memory_turns.len() + 1,
                &snapshot_record.snapshot_id,
                &snapshot_record.snapshot,
                None,
                &memory_refs,
                6,
            );
            memory_refs = open_turn.kept_refs.clone();
            memory_turns.push(open_turn);

            runtime
                .extract(
                    &mut session,
                    vec![ClaimInput {
                        id: format!("{}-{action_index}", step.1),
                        statement: step.2.to_string(),
                    }],
                    &extract_timestamp,
                )
                .expect("extract should work");
            let snapshot_record = session
                .snapshots
                .last()
                .expect("snapshot should remain current after extract");
            let report = session
                .evidence_reports
                .last()
                .expect("evidence report should exist");
            let extract_turn = plan_memory_turn(
                memory_turns.len() + 1,
                &snapshot_record.snapshot_id,
                &snapshot_record.snapshot,
                Some(&report.report),
                &memory_refs,
                6,
            );
            memory_refs = extract_turn.kept_refs.clone();
            memory_turns.push(extract_turn);
        }

        let summary = summarize_turns(&memory_turns, 12);
        let action_entries = session
            .transcript
            .entries
            .iter()
            .filter(|entry| entry.kind == TranscriptKind::Command)
            .count();

        assert_eq!(action_entries, 20);
        assert_eq!(summary.turn_count, 20);
        assert!(summary.max_working_set_size <= 6);
        assert!(summary.final_working_set_size <= 6);
        assert_eq!(summary.visited_urls.len(), 3);
        assert!(summary
            .synthesized_notes
            .iter()
            .any(|note| note == "The Starter plan costs $29 per month."));
        assert!(summary
            .synthesized_notes
            .iter()
            .any(|note| note == "Snapshot responses include stable refs and evidence metadata."));
    }

    #[test]
    fn synthesizes_multi_page_session_reports() {
        let runtime = ReadOnlyRuntime::default();
        let catalog = fixture_catalog();
        let mut session = runtime.start_session("ssynthesis001", "2026-03-14T00:00:00+09:00");

        runtime
            .open(
                &mut session,
                &catalog,
                "fixture://research/static-docs/getting-started",
                "2026-03-14T00:00:01+09:00",
            )
            .expect("open should work");
        runtime
            .extract(
                &mut session,
                vec![ClaimInput {
                    id: "claim-docs".to_string(),
                    statement:
                        "Touch Browser compiles web pages into semantic state for research agents."
                            .to_string(),
                }],
                "2026-03-14T00:00:02+09:00",
            )
            .expect("extract docs should work");
        runtime
            .open(
                &mut session,
                &catalog,
                "fixture://research/citation-heavy/pricing",
                "2026-03-14T00:00:03+09:00",
            )
            .expect("pricing open should work");
        runtime
            .extract(
                &mut session,
                vec![
                    ClaimInput {
                        id: "claim-pricing".to_string(),
                        statement: "The Starter plan costs $29 per month.".to_string(),
                    },
                    ClaimInput {
                        id: "claim-missing".to_string(),
                        statement: "The Enterprise plan starts at $9 per month.".to_string(),
                    },
                ],
                "2026-03-14T00:00:04+09:00",
            )
            .expect("pricing extract should work");

        let synthesis = runtime
            .synthesize_session(&session, "2026-03-14T00:00:05+09:00", 8)
            .expect("synthesis should work");

        assert_eq!(synthesis.snapshot_count, 2);
        assert_eq!(synthesis.evidence_report_count, 2);
        assert_eq!(synthesis.visited_urls.len(), 2);
        assert!(synthesis
            .supported_claims
            .iter()
            .any(|claim| claim.claim_id == "claim-docs"));
        assert!(synthesis
            .supported_claims
            .iter()
            .any(|claim| claim.claim_id == "claim-pricing"));
        assert!(synthesis
            .unsupported_claims
            .iter()
            .chain(synthesis.needs_more_browsing_claims.iter())
            .chain(synthesis.contradicted_claims.iter())
            .any(|claim| claim.claim_id == "claim-missing"));
        assert!(synthesis
            .synthesized_notes
            .iter()
            .any(|note| note.contains("Touch Browser compiles web pages")));
        assert!(synthesis
            .synthesized_notes
            .iter()
            .any(|note| note.contains("Starter plan costs $29")));
    }

    fn fixture_catalog() -> FixtureCatalog {
        let mut catalog = FixtureCatalog::default();

        for fixture_path in fixture_metadata_paths() {
            let metadata = read_fixture_metadata(&fixture_path);
            let html_path = repo_root().join(metadata.html_path);
            let html = fs::read_to_string(html_path).expect("fixture html should be readable");
            let risk = match metadata.risk.as_str() {
                "low" => SourceRisk::Low,
                "medium" => SourceRisk::Medium,
                "hostile" => SourceRisk::Hostile,
                other => panic!("unexpected risk: {other}"),
            };
            let aliases = match metadata.source_uri.as_str() {
                "fixture://research/static-docs/getting-started" => {
                    vec!["/docs".to_string(), "/getting-started".to_string()]
                }
                "fixture://research/citation-heavy/pricing" => vec!["/pricing".to_string()],
                "fixture://research/navigation/api-reference" => {
                    vec!["/api".to_string(), "/api-reference".to_string()]
                }
                _ => Vec::new(),
            };

            catalog.register(
                CatalogDocument::new(
                    metadata.source_uri,
                    html,
                    SourceType::Fixture,
                    risk,
                    Some(metadata.title),
                )
                .with_aliases(aliases),
            );
        }

        catalog
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should exist")
    }

    fn fixture_metadata_paths() -> Vec<PathBuf> {
        vec![
            repo_root().join("fixtures/research/static-docs/getting-started/fixture.json"),
            repo_root().join("fixtures/research/navigation/api-reference/fixture.json"),
            repo_root().join("fixtures/research/citation-heavy/pricing/fixture.json"),
        ]
    }

    fn read_fixture_metadata(path: &PathBuf) -> FixtureMetadata {
        serde_json::from_str(
            &fs::read_to_string(path).expect("fixture metadata should be readable"),
        )
        .expect("fixture metadata should deserialize")
    }

    struct LiveTestServer {
        base_url: String,
        stop_flag: Arc<AtomicBool>,
        handle: Option<thread::JoinHandle<()>>,
    }

    impl LiveTestServer {
        fn start(requests: Arc<AtomicUsize>) -> Self {
            let listener = TcpListener::bind("127.0.0.1:0").expect("listener bind");
            let address = listener.local_addr().expect("local addr");
            let server = Server::from_listener(listener, None).expect("server");
            let stop_flag = Arc::new(AtomicBool::new(false));
            let stop_flag_thread = stop_flag.clone();
            let base_url = format!("http://{}", address);

            let handle = thread::spawn(move || {
                while !stop_flag_thread.load(Ordering::SeqCst) {
                    let Ok(Some(request)) =
                        server.recv_timeout(std::time::Duration::from_millis(100))
                    else {
                        continue;
                    };
                    requests.fetch_add(1, Ordering::SeqCst);
                    let path = request.url().to_string();

                    let response = match path.as_str() {
                        "/robots.txt" => text_response("User-agent: *\nDisallow:\n", 200),
                        "/start" => redirect_response("/docs"),
                        "/docs" => html_response(
                            "<html><head><title>Live Docs</title></head><body><main><h1>Pricing</h1><p>The Starter plan costs $29 per month.</p></main></body></html>",
                            "text/html; charset=utf-8",
                            200,
                        ),
                        _ => html_response("<html><body>missing</body></html>", "text/html", 404),
                    };

                    let _ = request.respond(response);
                }
            });

            Self {
                base_url,
                stop_flag,
                handle: Some(handle),
            }
        }

        fn base_url(&self) -> &str {
            &self.base_url
        }
    }

    impl Drop for LiveTestServer {
        fn drop(&mut self) {
            self.stop_flag.store(true, Ordering::SeqCst);
            if let Some(handle) = self.handle.take() {
                let _ = handle.join();
            }
        }
    }

    fn html_response(body: &str, content_type: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
        let header = Header::from_bytes("Content-Type", content_type).expect("header");
        TinyResponse::new(
            StatusCode(status),
            vec![header],
            Cursor::new(body.as_bytes().to_vec()),
            Some(body.len()),
            None,
        )
    }

    fn text_response(body: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
        html_response(body, "text/plain; charset=utf-8", status)
    }

    fn redirect_response(location: &str) -> TinyResponse<Cursor<Vec<u8>>> {
        let header = Header::from_bytes("Location", location).expect("location");
        TinyResponse::new(
            StatusCode(302),
            vec![header],
            Cursor::new(Vec::new()),
            Some(0),
            None,
        )
    }
}
