use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use touch_browser_acquisition::{AcquisitionConfig, AcquisitionEngine};
use touch_browser_contracts::{
    EvidenceReport, EvidenceVerificationReport, SearchReport, SnapshotDocument, SourceRisk,
};
use touch_browser_runtime::{ClaimInput, FixtureCatalog, ReadOnlySession};
use touch_browser_storage_sqlite::{PilotTelemetryEvent, PilotTelemetrySummary};

use crate::{
    application::{
        browser_session::{
            BrowserActionSource, BrowserActionTraceEntry, BrowserCliSession,
            BrowserLoadDiagnostics, BrowserOrigin, BrowserSessionContext, PersistedBrowserState,
        },
        ports::*,
    },
    infrastructure::{
        browser_models::{
            PlaywrightClickParams, PlaywrightExpandParams, PlaywrightFollowParams,
            PlaywrightPaginateParams, PlaywrightSnapshotParams, PlaywrightSubmitParams,
            PlaywrightTypeParams, PlaywrightTypePrefill,
        },
        browser_runtime::{
            browser_context_dir_for_session_file, browser_secret_store_path,
            build_browser_cli_session, collect_submit_prefill, compile_browser_snapshot,
            current_browser_action_source, current_snapshot_ref_dom_path_hint,
            current_snapshot_ref_href, current_snapshot_ref_input_type,
            current_snapshot_ref_is_sensitive, current_snapshot_ref_name,
            current_snapshot_ref_tag_name, current_snapshot_ref_text, invoke_playwright_click,
            invoke_playwright_expand, invoke_playwright_follow, invoke_playwright_paginate,
            invoke_playwright_snapshot, invoke_playwright_submit, invoke_playwright_type,
            load_browser_cli_secrets, load_browser_cli_session, mark_browser_session_interactive,
            next_session_timestamp, open_browser_session, resolved_browser_source_url,
            save_browser_cli_secrets, save_browser_cli_session, stable_ref_ordinal_hint,
        },
        fixtures::load_fixture_catalog,
    },
    interface::{cli_error::CliError, cli_models::SecretPrefill},
};

pub(crate) struct DefaultSessionStore;
pub(crate) struct DefaultBrowserAutomation;
pub(crate) struct DefaultFixtureCatalog;
pub(crate) struct DefaultAcquisitionFactory;
pub(crate) struct DefaultEvidenceVerifier;
pub(crate) struct DefaultTelemetry;

pub(crate) static DEFAULT_SESSION_STORE: DefaultSessionStore = DefaultSessionStore;
pub(crate) static DEFAULT_BROWSER_AUTOMATION: DefaultBrowserAutomation = DefaultBrowserAutomation;
pub(crate) static DEFAULT_FIXTURE_CATALOG: DefaultFixtureCatalog = DefaultFixtureCatalog;
pub(crate) static DEFAULT_ACQUISITION_FACTORY: DefaultAcquisitionFactory =
    DefaultAcquisitionFactory;
pub(crate) static DEFAULT_EVIDENCE_VERIFIER: DefaultEvidenceVerifier = DefaultEvidenceVerifier;
pub(crate) static DEFAULT_TELEMETRY: DefaultTelemetry = DefaultTelemetry;

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

    fn snapshot_reference(
        &self,
        session: &ReadOnlySession,
        target_ref: &str,
    ) -> Result<BrowserSnapshotReference, CliError> {
        Ok(BrowserSnapshotReference {
            target_ref: target_ref.to_string(),
            text: current_snapshot_ref_text(session, target_ref)?,
            href: current_snapshot_ref_href(session, target_ref),
            tag_name: current_snapshot_ref_tag_name(session, target_ref),
            dom_path_hint: current_snapshot_ref_dom_path_hint(session, target_ref),
            ordinal_hint: stable_ref_ordinal_hint(target_ref),
            name: current_snapshot_ref_name(session, target_ref),
            input_type: current_snapshot_ref_input_type(session, target_ref),
            sensitive: current_snapshot_ref_is_sensitive(session, target_ref),
        })
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
        request: BrowserSnapshotCaptureRequest,
    ) -> Result<BrowserSnapshotCaptureResult, CliError> {
        let result = invoke_playwright_snapshot(PlaywrightSnapshotParams {
            url: request.url,
            html: request.html,
            context_dir: request.context_dir,
            profile_dir: request.profile_dir,
            budget: request.budget,
            headless: request.headless,
            search_identity: request.search_identity,
            manual_recovery: false,
        })?;
        Ok(BrowserSnapshotCaptureResult {
            final_url: result.final_url,
            html: result.html,
            load_diagnostics: BrowserLoadDiagnostics {
                wait_strategy: result.diagnostics.wait_strategy,
                wait_budget_ms: result.diagnostics.wait_budget_ms,
                wait_consumed_ms: result.diagnostics.wait_consumed_ms,
                wait_stop_reason: result.diagnostics.wait_stop_reason,
            },
        })
    }

    fn invoke_follow(
        &self,
        request: BrowserFollowRequest,
    ) -> Result<BrowserFollowResult, CliError> {
        if let Some(result) = fixture_follow_result(&request)? {
            return Ok(result);
        }

        let result = invoke_playwright_follow(PlaywrightFollowParams {
            url: request.source.url,
            html: request.source.html,
            context_dir: request.source.context_dir,
            profile_dir: request.source.profile_dir,
            target_ref: request.target.target_ref,
            target_text: request.target.text,
            target_href: request.target.href,
            target_tag_name: request.target.tag_name,
            target_dom_path_hint: request.target.dom_path_hint,
            target_ordinal_hint: request.target.ordinal_hint,
            headless: request.headless,
        })?;
        Ok(BrowserFollowResult {
            followed_ref: result.followed_ref,
            target_text: result.target_text,
            target_href: result.target_href,
            clicked_text: result.clicked_text,
            final_url: result.final_url,
            title: result.title,
            visible_text: result.visible_text,
            html: result.html,
            load_diagnostics: BrowserLoadDiagnostics {
                wait_strategy: result.diagnostics.wait_strategy,
                wait_budget_ms: result.diagnostics.wait_budget_ms,
                wait_consumed_ms: result.diagnostics.wait_consumed_ms,
                wait_stop_reason: result.diagnostics.wait_stop_reason,
            },
        })
    }

    fn invoke_click(&self, request: BrowserClickRequest) -> Result<BrowserClickResult, CliError> {
        let result = invoke_playwright_click(PlaywrightClickParams {
            url: request.source.url,
            html: request.source.html,
            context_dir: request.source.context_dir,
            profile_dir: request.source.profile_dir,
            target_ref: request.target.target_ref,
            target_text: request.target.text,
            target_href: request.target.href,
            target_tag_name: request.target.tag_name,
            target_dom_path_hint: request.target.dom_path_hint,
            target_ordinal_hint: request.target.ordinal_hint,
            headless: request.headless,
        })?;
        Ok(BrowserClickResult {
            clicked_ref: result.clicked_ref,
            target_text: result.target_text,
            target_href: result.target_href,
            clicked_text: result.clicked_text,
            final_url: result.final_url,
            title: result.title,
            visible_text: result.visible_text,
            html: result.html,
            load_diagnostics: BrowserLoadDiagnostics {
                wait_strategy: result.diagnostics.wait_strategy,
                wait_budget_ms: result.diagnostics.wait_budget_ms,
                wait_consumed_ms: result.diagnostics.wait_consumed_ms,
                wait_stop_reason: result.diagnostics.wait_stop_reason,
            },
        })
    }

    fn invoke_type(&self, request: BrowserTypeRequest) -> Result<BrowserTypeResult, CliError> {
        let result = invoke_playwright_type(PlaywrightTypeParams {
            url: request.source.url,
            html: request.source.html,
            context_dir: request.source.context_dir,
            profile_dir: request.source.profile_dir,
            target_ref: request.target.target_ref,
            target_text: request.target.text,
            target_tag_name: request.target.tag_name,
            target_dom_path_hint: request.target.dom_path_hint,
            target_ordinal_hint: request.target.ordinal_hint,
            target_name: request.target.name,
            target_input_type: request.target.input_type,
            value: request.value,
            headless: request.headless,
        })?;
        Ok(BrowserTypeResult {
            typed_ref: result.typed_ref,
            target_text: result.target_text,
            typed_length: result.typed_length,
            final_url: result.final_url,
            title: result.title,
            visible_text: result.visible_text,
            html: result.html,
            load_diagnostics: BrowserLoadDiagnostics {
                wait_strategy: result.diagnostics.wait_strategy,
                wait_budget_ms: result.diagnostics.wait_budget_ms,
                wait_consumed_ms: result.diagnostics.wait_consumed_ms,
                wait_stop_reason: result.diagnostics.wait_stop_reason,
            },
        })
    }

    fn invoke_submit(
        &self,
        request: BrowserSubmitRequest,
    ) -> Result<BrowserSubmitResult, CliError> {
        let result = invoke_playwright_submit(PlaywrightSubmitParams {
            url: request.source.url,
            html: request.source.html,
            context_dir: request.source.context_dir,
            profile_dir: request.source.profile_dir,
            target_ref: request.target.target_ref,
            target_text: request.target.text,
            target_tag_name: request.target.tag_name,
            target_dom_path_hint: request.target.dom_path_hint,
            target_ordinal_hint: request.target.ordinal_hint,
            prefill: request
                .prefill
                .into_iter()
                .map(|prefill| PlaywrightTypePrefill {
                    target_ref: prefill.target_ref,
                    target_text: prefill.target_text,
                    target_tag_name: prefill.target_tag_name,
                    target_dom_path_hint: prefill.target_dom_path_hint,
                    target_ordinal_hint: prefill.target_ordinal_hint,
                    target_name: prefill.target_name,
                    target_input_type: prefill.target_input_type,
                    value: prefill.value,
                })
                .collect(),
            headless: request.headless,
        })?;
        Ok(BrowserSubmitResult {
            submitted_ref: result.submitted_ref,
            target_text: result.target_text,
            final_url: result.final_url,
            title: result.title,
            visible_text: result.visible_text,
            html: result.html,
        })
    }

    fn invoke_paginate(
        &self,
        request: BrowserPaginateRequest,
    ) -> Result<BrowserPaginateResult, CliError> {
        let result = invoke_playwright_paginate(PlaywrightPaginateParams {
            url: request.source.url,
            html: request.source.html,
            context_dir: request.source.context_dir,
            profile_dir: request.source.profile_dir,
            direction: request.direction,
            current_page: request.current_page,
            headless: request.headless,
        })?;
        Ok(BrowserPaginateResult {
            page: result.page,
            clicked_text: result.clicked_text,
            final_url: result.final_url,
            title: result.title,
            visible_text: result.visible_text,
            html: result.html,
        })
    }

    fn invoke_expand(
        &self,
        request: BrowserExpandRequest,
    ) -> Result<BrowserExpandResult, CliError> {
        let result = invoke_playwright_expand(PlaywrightExpandParams {
            url: request.source.url,
            html: request.source.html,
            context_dir: request.source.context_dir,
            profile_dir: request.source.profile_dir,
            target_ref: request.target.target_ref,
            target_text: request.target.text,
            target_tag_name: request.target.tag_name,
            target_dom_path_hint: request.target.dom_path_hint,
            target_ordinal_hint: request.target.ordinal_hint,
            headless: request.headless,
        })?;
        Ok(BrowserExpandResult {
            expanded_ref: result.expanded_ref,
            target_text: result.target_text,
            clicked_text: result.clicked_text,
            final_url: result.final_url,
            title: result.title,
            visible_text: result.visible_text,
            html: result.html,
        })
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

    fn collect_submit_prefill(
        &self,
        persisted: &BrowserCliSession,
        extra_prefill: &[SecretPrefill],
    ) -> Vec<BrowserSubmitPrefill> {
        collect_submit_prefill(persisted, extra_prefill)
            .into_iter()
            .map(|prefill| BrowserSubmitPrefill {
                target_ref: prefill.target_ref,
                target_text: prefill.target_text,
                target_tag_name: prefill.target_tag_name,
                target_dom_path_hint: prefill.target_dom_path_hint,
                target_ordinal_hint: prefill.target_ordinal_hint,
                target_name: prefill.target_name,
                target_input_type: prefill.target_input_type,
                value: prefill.value,
            })
            .collect()
    }

    fn mark_browser_session_interactive(&self, persisted: &mut BrowserCliSession) {
        mark_browser_session_interactive(persisted)
    }
}

fn fixture_follow_result(
    request: &BrowserFollowRequest,
) -> Result<Option<BrowserFollowResult>, CliError> {
    let Some(href) = request.target.href.as_deref() else {
        return Ok(None);
    };
    if !request.source.source_url.starts_with("fixture://") || href.starts_with('#') {
        return Ok(None);
    }

    let catalog = load_fixture_catalog()?;
    let Some(document) = catalog.resolve_link(&request.source.source_url, href) else {
        return Ok(None);
    };
    if document.source_url == request.source.source_url {
        return Ok(None);
    }

    let label = document
        .source_label
        .clone()
        .unwrap_or_else(|| document.source_url.clone());
    Ok(Some(BrowserFollowResult {
        followed_ref: request.target.target_ref.clone(),
        target_text: request.target.text.clone(),
        target_href: request.target.href.clone(),
        clicked_text: request.target.text.clone(),
        final_url: document.source_url.clone(),
        title: label.clone(),
        visible_text: label,
        html: document.html.clone(),
        load_diagnostics: BrowserLoadDiagnostics {
            wait_strategy: "fixture-link-alias".to_string(),
            wait_budget_ms: None,
            wait_consumed_ms: Some(0),
            wait_stop_reason: Some("fixture-catalog".to_string()),
        },
    }))
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

impl EvidenceVerifierPort for DefaultEvidenceVerifier {
    fn run_verifier(
        &self,
        verifier_command: &str,
        claims: &[ClaimInput],
        snapshot: &SnapshotDocument,
        report: &EvidenceReport,
        generated_at: &str,
    ) -> Result<EvidenceVerificationReport, CliError> {
        super::verifier::run_verifier_command(
            verifier_command,
            claims,
            snapshot,
            report,
            generated_at,
        )
    }
}

impl TelemetryPort for DefaultTelemetry {
    fn summary(&self) -> Result<PilotTelemetrySummary, CliError> {
        super::telemetry::telemetry_store()?
            .summary()
            .map_err(Into::into)
    }

    fn recent_events(&self, limit: usize) -> Result<Vec<PilotTelemetryEvent>, CliError> {
        super::telemetry::telemetry_store()?
            .recent_events(limit)
            .map_err(Into::into)
    }
}
