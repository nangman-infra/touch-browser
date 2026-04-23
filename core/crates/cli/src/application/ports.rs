use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use touch_browser_acquisition::AcquisitionEngine;
use touch_browser_storage_sqlite::{PilotTelemetryEvent, PilotTelemetrySummary};

use super::{
    browser_session::{
        BrowserActionSource, BrowserActionTraceEntry, BrowserCliSession, BrowserLoadDiagnostics,
        BrowserOrigin, BrowserSessionContext, PersistedBrowserState,
    },
    deps::{
        ClaimInput, CliError, EvidenceReport, EvidenceVerificationReport, FixtureCatalog,
        ReadOnlySession, SearchReport, SecretPrefill, SnapshotDocument, SourceRisk,
    },
};

#[derive(Debug, Clone)]
pub(crate) struct BrowserSnapshotReference {
    pub(crate) target_ref: String,
    pub(crate) text: String,
    pub(crate) href: Option<String>,
    pub(crate) tag_name: Option<String>,
    pub(crate) dom_path_hint: Option<String>,
    pub(crate) ordinal_hint: Option<usize>,
    pub(crate) name: Option<String>,
    pub(crate) input_type: Option<String>,
    pub(crate) sensitive: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserSnapshotCaptureRequest {
    pub(crate) url: Option<String>,
    pub(crate) html: Option<String>,
    pub(crate) context_dir: Option<String>,
    pub(crate) profile_dir: Option<String>,
    pub(crate) budget: usize,
    pub(crate) headless: bool,
    pub(crate) search_identity: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserSnapshotCaptureResult {
    pub(crate) final_url: String,
    pub(crate) html: String,
    #[allow(dead_code)]
    pub(crate) load_diagnostics: BrowserLoadDiagnostics,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserSubmitPrefill {
    pub(crate) target_ref: String,
    pub(crate) target_text: Option<String>,
    pub(crate) target_tag_name: Option<String>,
    pub(crate) target_dom_path_hint: Option<String>,
    pub(crate) target_ordinal_hint: Option<usize>,
    pub(crate) target_name: Option<String>,
    pub(crate) target_input_type: Option<String>,
    pub(crate) value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserFollowRequest {
    pub(crate) source: BrowserActionSource,
    pub(crate) target: BrowserSnapshotReference,
    pub(crate) headless: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserFollowResult {
    pub(crate) followed_ref: String,
    pub(crate) target_text: String,
    pub(crate) target_href: Option<String>,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
    pub(crate) load_diagnostics: BrowserLoadDiagnostics,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserDownloadEvidence {
    pub(crate) completed: bool,
    pub(crate) suggested_filename: String,
    pub(crate) path: Option<String>,
    pub(crate) byte_length: Option<u64>,
    pub(crate) sha256: Option<String>,
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserClickRequest {
    pub(crate) source: BrowserActionSource,
    pub(crate) target: BrowserSnapshotReference,
    pub(crate) headless: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserClickResult {
    pub(crate) clicked_ref: String,
    pub(crate) target_text: String,
    pub(crate) target_href: Option<String>,
    pub(crate) clicked_text: String,
    pub(crate) download: Option<BrowserDownloadEvidence>,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
    pub(crate) load_diagnostics: BrowserLoadDiagnostics,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserTypeRequest {
    pub(crate) source: BrowserActionSource,
    pub(crate) target: BrowserSnapshotReference,
    pub(crate) value: String,
    pub(crate) headless: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserTypeResult {
    pub(crate) typed_ref: String,
    pub(crate) target_text: String,
    pub(crate) typed_length: usize,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
    pub(crate) load_diagnostics: BrowserLoadDiagnostics,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserSubmitRequest {
    pub(crate) source: BrowserActionSource,
    pub(crate) target: BrowserSnapshotReference,
    pub(crate) prefill: Vec<BrowserSubmitPrefill>,
    pub(crate) headless: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserSubmitResult {
    pub(crate) submitted_ref: String,
    pub(crate) target_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserPaginateRequest {
    pub(crate) source: BrowserActionSource,
    pub(crate) direction: String,
    pub(crate) current_page: usize,
    pub(crate) headless: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserPaginateResult {
    pub(crate) page: usize,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserExpandRequest {
    pub(crate) source: BrowserActionSource,
    pub(crate) target: BrowserSnapshotReference,
    pub(crate) headless: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserExpandResult {
    pub(crate) expanded_ref: String,
    pub(crate) target_text: String,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

pub(crate) trait SessionStorePort {
    fn save_session(&self, path: &Path, persisted: &BrowserCliSession) -> Result<(), CliError>;
    fn load_session(&self, path: &Path) -> Result<BrowserCliSession, CliError>;
    fn browser_context_dir_for_session(&self, path: &Path) -> PathBuf;
    fn secret_store_path(&self, path: &Path) -> PathBuf;
    fn load_secrets(&self, path: &Path) -> Result<BTreeMap<String, String>, CliError>;
    fn save_secrets(&self, path: &Path, secrets: &BTreeMap<String, String>)
        -> Result<(), CliError>;
}

pub(crate) trait BrowserAutomationPort {
    #[allow(clippy::too_many_arguments)]
    fn open_browser_session(
        &self,
        target: &str,
        requested_budget: usize,
        source_risk: Option<SourceRisk>,
        source_label: Option<String>,
        headed: bool,
        browser_context_dir: Option<String>,
        browser_profile_dir: Option<String>,
        session_id: &str,
        timestamp: &str,
    ) -> Result<BrowserSessionContext, CliError>;

    fn current_browser_action_source(
        &self,
        persisted: &BrowserCliSession,
    ) -> Result<BrowserActionSource, CliError>;
    fn snapshot_reference(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Result<BrowserSnapshotReference, CliError>;
    fn resolved_browser_source_url(&self, source: &BrowserActionSource, final_url: &str) -> String;
    fn compile_snapshot(
        &self,
        source_url: &str,
        html: &str,
        requested_tokens: usize,
    ) -> Result<SnapshotDocument, CliError>;
    fn invoke_snapshot(
        &self,
        request: BrowserSnapshotCaptureRequest,
    ) -> Result<BrowserSnapshotCaptureResult, CliError>;
    fn invoke_follow(&self, request: BrowserFollowRequest)
        -> Result<BrowserFollowResult, CliError>;
    fn invoke_click(&self, request: BrowserClickRequest) -> Result<BrowserClickResult, CliError>;
    fn invoke_type(&self, request: BrowserTypeRequest) -> Result<BrowserTypeResult, CliError>;
    fn invoke_submit(&self, request: BrowserSubmitRequest)
        -> Result<BrowserSubmitResult, CliError>;
    fn invoke_paginate(
        &self,
        request: BrowserPaginateRequest,
    ) -> Result<BrowserPaginateResult, CliError>;
    fn invoke_expand(&self, request: BrowserExpandRequest)
        -> Result<BrowserExpandResult, CliError>;
    #[allow(clippy::too_many_arguments)]
    fn build_browser_cli_session(
        &self,
        session: &ReadOnlySession,
        requested_budget: usize,
        headless: bool,
        browser_state: Option<PersistedBrowserState>,
        browser_context_dir: Option<String>,
        browser_profile_dir: Option<String>,
        browser_origin: Option<BrowserOrigin>,
        allowlisted_domains: Vec<String>,
        browser_trace: Vec<BrowserActionTraceEntry>,
        latest_search: Option<SearchReport>,
    ) -> BrowserCliSession;
    fn next_session_timestamp(&self, session: &ReadOnlySession) -> String;
    fn collect_submit_prefill(
        &self,
        persisted: &BrowserCliSession,
        extra_prefill: &[SecretPrefill],
    ) -> Vec<BrowserSubmitPrefill>;
    fn mark_browser_session_interactive(&self, persisted: &mut BrowserCliSession);
}

pub(crate) trait FixtureCatalogPort {
    fn load_catalog(&self) -> Result<FixtureCatalog, CliError>;
}

pub(crate) trait AcquisitionFactoryPort {
    fn create_engine(&self) -> Result<AcquisitionEngine, CliError>;
}

pub(crate) trait EvidenceVerifierPort {
    fn run_verifier(
        &self,
        verifier_command: &str,
        claims: &[ClaimInput],
        snapshot: &SnapshotDocument,
        report: &EvidenceReport,
        generated_at: &str,
    ) -> Result<EvidenceVerificationReport, CliError>;
}

pub(crate) trait TelemetryPort {
    fn summary(&self) -> Result<PilotTelemetrySummary, CliError>;
    fn recent_events(&self, limit: usize) -> Result<Vec<PilotTelemetryEvent>, CliError>;
}

#[derive(Clone, Copy)]
pub(crate) struct CliPorts<'a> {
    pub(crate) session_store: &'a dyn SessionStorePort,
    pub(crate) browser: &'a dyn BrowserAutomationPort,
    pub(crate) fixtures: &'a dyn FixtureCatalogPort,
    pub(crate) acquisition: &'a dyn AcquisitionFactoryPort,
    pub(crate) verifier: &'a dyn EvidenceVerifierPort,
    pub(crate) telemetry: &'a dyn TelemetryPort,
}
