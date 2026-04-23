use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use touch_browser_contracts::{
    ReplayTranscript, SessionState, SnapshotDocument, SourceRisk, SourceType,
};
use touch_browser_evidence::EvidenceExtractor;
use touch_browser_observation::ObservationCompiler;

mod session_claims;
mod session_extract;
mod session_journal;
mod session_loading;
mod session_navigation;
mod session_replay;
mod session_start;
mod session_synthesis;

use session_claims::aggregate_session_claims;

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

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("unknown source url: {0}")]
    UnknownSource(String),
    #[error("no current snapshot is available")]
    NoCurrentSnapshot,
    #[error("missing current url")]
    MissingCurrentUrl,
    #[error("target ref not found in current snapshot: {0}")]
    MissingRef(String),
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
