use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use touch_browser_acquisition::AcquisitionEngine;

use crate::*;

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
    fn resolved_browser_source_url(&self, source: &BrowserActionSource, final_url: &str) -> String;
    fn compile_snapshot(
        &self,
        source_url: &str,
        html: &str,
        requested_tokens: usize,
    ) -> Result<SnapshotDocument, CliError>;
    fn invoke_snapshot(
        &self,
        params: PlaywrightSnapshotParams,
    ) -> Result<PlaywrightSnapshotResult, CliError>;
    fn invoke_follow(
        &self,
        params: PlaywrightFollowParams,
    ) -> Result<PlaywrightFollowResult, CliError>;
    fn invoke_click(
        &self,
        params: PlaywrightClickParams,
    ) -> Result<PlaywrightClickResult, CliError>;
    fn invoke_type(&self, params: PlaywrightTypeParams) -> Result<PlaywrightTypeResult, CliError>;
    fn invoke_submit(
        &self,
        params: PlaywrightSubmitParams,
    ) -> Result<PlaywrightSubmitResult, CliError>;
    fn invoke_paginate(
        &self,
        params: PlaywrightPaginateParams,
    ) -> Result<PlaywrightPaginateResult, CliError>;
    fn invoke_expand(
        &self,
        params: PlaywrightExpandParams,
    ) -> Result<PlaywrightExpandResult, CliError>;
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
    fn stable_ref_ordinal_hint(&self, target_ref: &str) -> Option<usize>;
    fn current_snapshot_ref_text(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Result<String, CliError>;
    fn current_snapshot_ref_href(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String>;
    fn current_snapshot_ref_tag_name(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String>;
    fn current_snapshot_ref_dom_path_hint(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String>;
    fn current_snapshot_ref_name(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String>;
    fn current_snapshot_ref_input_type(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String>;
    fn current_snapshot_ref_is_sensitive(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> bool;
    fn collect_submit_prefill(
        &self,
        persisted: &BrowserCliSession,
        extra_prefill: &[SecretPrefill],
    ) -> Vec<PlaywrightTypePrefill>;
    fn mark_browser_session_interactive(&self, persisted: &mut BrowserCliSession);
}

pub(crate) trait FixtureCatalogPort {
    fn load_catalog(&self) -> Result<FixtureCatalog, CliError>;
}

pub(crate) trait AcquisitionFactoryPort {
    fn create_engine(&self) -> Result<AcquisitionEngine, CliError>;
}

#[derive(Clone, Copy)]
pub(crate) struct CliPorts<'a> {
    pub(crate) session_store: &'a dyn SessionStorePort,
    pub(crate) browser: &'a dyn BrowserAutomationPort,
    pub(crate) fixtures: &'a dyn FixtureCatalogPort,
    pub(crate) acquisition: &'a dyn AcquisitionFactoryPort,
}

pub(crate) fn default_cli_ports() -> CliPorts<'static> {
    CliPorts {
        session_store: &crate::infrastructure::app_ports::DEFAULT_SESSION_STORE,
        browser: &crate::infrastructure::app_ports::DEFAULT_BROWSER_AUTOMATION,
        fixtures: &crate::infrastructure::app_ports::DEFAULT_FIXTURE_CATALOG,
        acquisition: &crate::infrastructure::app_ports::DEFAULT_ACQUISITION_FACTORY,
    }
}
