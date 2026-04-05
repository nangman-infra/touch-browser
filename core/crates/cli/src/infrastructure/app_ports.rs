use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use touch_browser_acquisition::{AcquisitionConfig, AcquisitionEngine};

use crate::{application::ports::*, *};

pub(crate) struct DefaultSessionStore;
pub(crate) struct DefaultBrowserAutomation;
pub(crate) struct DefaultFixtureCatalog;
pub(crate) struct DefaultAcquisitionFactory;

pub(crate) static DEFAULT_SESSION_STORE: DefaultSessionStore = DefaultSessionStore;
pub(crate) static DEFAULT_BROWSER_AUTOMATION: DefaultBrowserAutomation = DefaultBrowserAutomation;
pub(crate) static DEFAULT_FIXTURE_CATALOG: DefaultFixtureCatalog = DefaultFixtureCatalog;
pub(crate) static DEFAULT_ACQUISITION_FACTORY: DefaultAcquisitionFactory =
    DefaultAcquisitionFactory;

impl SessionStorePort for DefaultSessionStore {
    fn save_session(&self, path: &Path, persisted: &BrowserCliSession) -> Result<(), CliError> {
        save_browser_cli_session(path, persisted)
    }

    fn load_session(&self, path: &Path) -> Result<BrowserCliSession, CliError> {
        load_browser_cli_session(path)
    }

    fn browser_context_dir_for_session(&self, path: &Path) -> PathBuf {
        browser_context_dir_for_session_file(path)
    }

    fn secret_store_path(&self, path: &Path) -> PathBuf {
        browser_secret_store_path(path)
    }

    fn load_secrets(&self, path: &Path) -> Result<BTreeMap<String, String>, CliError> {
        load_browser_cli_secrets(path)
    }

    fn save_secrets(
        &self,
        path: &Path,
        secrets: &BTreeMap<String, String>,
    ) -> Result<(), CliError> {
        save_browser_cli_secrets(path, secrets)
    }
}

impl BrowserAutomationPort for DefaultBrowserAutomation {
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
    ) -> Result<BrowserSessionContext, CliError> {
        open_browser_session(
            target,
            requested_budget,
            source_risk,
            source_label,
            headed,
            browser_context_dir,
            browser_profile_dir,
            session_id,
            timestamp,
        )
    }

    fn current_browser_action_source(
        &self,
        persisted: &BrowserCliSession,
    ) -> Result<BrowserActionSource, CliError> {
        current_browser_action_source(persisted)
    }

    fn resolved_browser_source_url(&self, source: &BrowserActionSource, final_url: &str) -> String {
        resolved_browser_source_url(source, final_url)
    }

    fn compile_snapshot(
        &self,
        source_url: &str,
        html: &str,
        requested_tokens: usize,
    ) -> Result<SnapshotDocument, CliError> {
        compile_browser_snapshot(source_url, html, requested_tokens)
    }

    fn invoke_snapshot(
        &self,
        params: PlaywrightSnapshotParams,
    ) -> Result<PlaywrightSnapshotResult, CliError> {
        invoke_playwright_snapshot(params)
    }

    fn invoke_follow(
        &self,
        params: PlaywrightFollowParams,
    ) -> Result<PlaywrightFollowResult, CliError> {
        invoke_playwright_follow(params)
    }

    fn invoke_click(
        &self,
        params: PlaywrightClickParams,
    ) -> Result<PlaywrightClickResult, CliError> {
        invoke_playwright_click(params)
    }

    fn invoke_type(&self, params: PlaywrightTypeParams) -> Result<PlaywrightTypeResult, CliError> {
        invoke_playwright_type(params)
    }

    fn invoke_submit(
        &self,
        params: PlaywrightSubmitParams,
    ) -> Result<PlaywrightSubmitResult, CliError> {
        invoke_playwright_submit(params)
    }

    fn invoke_paginate(
        &self,
        params: PlaywrightPaginateParams,
    ) -> Result<PlaywrightPaginateResult, CliError> {
        invoke_playwright_paginate(params)
    }

    fn invoke_expand(
        &self,
        params: PlaywrightExpandParams,
    ) -> Result<PlaywrightExpandResult, CliError> {
        invoke_playwright_expand(params)
    }

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
    ) -> BrowserCliSession {
        build_browser_cli_session(
            session,
            requested_budget,
            headless,
            browser_state,
            browser_context_dir,
            browser_profile_dir,
            browser_origin,
            allowlisted_domains,
            browser_trace,
            latest_search,
        )
    }

    fn next_session_timestamp(&self, session: &ReadOnlySession) -> String {
        next_session_timestamp(session)
    }

    fn stable_ref_ordinal_hint(&self, target_ref: &str) -> Option<usize> {
        stable_ref_ordinal_hint(target_ref)
    }

    fn current_snapshot_ref_text(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Result<String, CliError> {
        current_snapshot_ref_text(session, target_ref)
    }

    fn current_snapshot_ref_href(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String> {
        current_snapshot_ref_href(session, target_ref)
    }

    fn current_snapshot_ref_tag_name(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String> {
        current_snapshot_ref_tag_name(session, target_ref)
    }

    fn current_snapshot_ref_dom_path_hint(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String> {
        current_snapshot_ref_dom_path_hint(session, target_ref)
    }

    fn current_snapshot_ref_name(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String> {
        current_snapshot_ref_name(session, target_ref)
    }

    fn current_snapshot_ref_input_type(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Option<String> {
        current_snapshot_ref_input_type(session, target_ref)
    }

    fn current_snapshot_ref_is_sensitive(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> bool {
        current_snapshot_ref_is_sensitive(session, target_ref)
    }

    fn collect_submit_prefill(
        &self,
        persisted: &BrowserCliSession,
        extra_prefill: &[SecretPrefill],
    ) -> Vec<PlaywrightTypePrefill> {
        collect_submit_prefill(persisted, extra_prefill)
    }

    fn mark_browser_session_interactive(&self, persisted: &mut BrowserCliSession) {
        mark_browser_session_interactive(persisted)
    }
}

impl FixtureCatalogPort for DefaultFixtureCatalog {
    fn load_catalog(&self) -> Result<FixtureCatalog, CliError> {
        load_fixture_catalog()
    }
}

impl AcquisitionFactoryPort for DefaultAcquisitionFactory {
    fn create_engine(&self) -> Result<AcquisitionEngine, CliError> {
        AcquisitionEngine::new(AcquisitionConfig::default()).map_err(CliError::Acquisition)
    }
}
