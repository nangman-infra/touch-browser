use touch_browser_action_vm::ReadOnlyActionVm;
use touch_browser_contracts::{
    ActionCommand, ActionFailureKind, ActionName, ActionResult, ActionStatus, EvidenceCitation,
    EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceReport, EvidenceVerificationOutcome,
    EvidenceVerificationReport, EvidenceVerificationVerdict, PolicyProfile, PolicyReport,
    ReplayTranscript, RiskClass, SearchActionActor, SearchActionHint, SearchEngine, SearchReport,
    SearchReportStatus, SearchResultItem, SessionMode, SessionSynthesisClaim,
    SessionSynthesisClaimStatus, SessionSynthesisReport, SnapshotBlock, SnapshotBlockKind,
    SnapshotBlockRole, SnapshotDocument, SourceRisk, SourceType, UnsupportedClaimReason,
    CONTRACT_VERSION,
};
use touch_browser_memory::{plan_memory_turn, summarize_turns};
use touch_browser_observation::{
    recommend_requested_tokens, ObservationCompiler, ObservationInput,
};
use touch_browser_policy::PolicyKernel;
use touch_browser_runtime::{
    CatalogDocument, ClaimInput, FixtureCatalog, ReadOnlyRuntime, ReadOnlySession, RuntimeError,
};

pub(crate) mod application;
pub(crate) mod infrastructure;
pub(crate) mod interface;

pub(crate) use application::policy_support::{
    approved_risk_labels, checkpoint_approval_panel, checkpoint_candidates, checkpoint_playbook,
    checkpoint_provider_hints, current_policy_with_allowlist, fail_action, merge_ack_risks,
    policy_profile_label, preflight_interactive_action, preflight_ref_action,
    preflight_session_block, promoted_policy_profile_for_risks, recommended_policy_profile,
    reject_action, required_ack_risks, succeed_action, InteractivePreflightOptions,
};
pub(crate) use application::presentation_support::render_session_synthesis_markdown;
pub(crate) use application::search_support::{
    is_search_results_target, resolve_latest_search_session_file,
};
pub(crate) use application::session_reporting::verify_action_result_if_requested;
pub(crate) use infrastructure::browser_models::{
    BrowserActionSource, BrowserActionTraceEntry, BrowserCliSession, BrowserOrigin,
    BrowserSessionContext, PersistedBrowserState, PlaywrightClickParams, PlaywrightExpandParams,
    PlaywrightFollowParams, PlaywrightPaginateParams, PlaywrightSnapshotParams,
    PlaywrightSubmitParams, PlaywrightTypeParams, PlaywrightTypePrefill,
};
pub(crate) use infrastructure::browser_runtime::{
    browser_context_dir_for_session_file, browser_secret_store_path, build_browser_cli_session,
    collect_submit_prefill, compile_browser_snapshot, current_browser_action_source,
    current_snapshot_ref_dom_path_hint, current_snapshot_ref_href, current_snapshot_ref_input_type,
    current_snapshot_ref_is_sensitive, current_snapshot_ref_name, current_snapshot_ref_tag_name,
    current_snapshot_ref_text, invoke_playwright_click, invoke_playwright_expand,
    invoke_playwright_follow, invoke_playwright_paginate, invoke_playwright_snapshot,
    invoke_playwright_submit, invoke_playwright_type, load_browser_cli_secrets,
    load_browser_cli_session, mark_browser_session_interactive, next_session_timestamp,
    open_browser_session, resolved_browser_source_url, save_browser_cli_secrets,
    save_browser_cli_session, stable_ref_ordinal_hint,
};
pub(crate) use infrastructure::fixtures::load_fixture_catalog;
pub(crate) use infrastructure::telemetry::{
    log_telemetry_error, log_telemetry_success, telemetry_store, telemetry_surface_label,
};
pub(crate) use interface::cli_dispatch::{dispatch, run_serve};
#[cfg(test)]
pub(crate) use interface::cli_entry::{command_usage, preprocess_cli_args};
#[cfg(test)]
pub(crate) use interface::cli_error::build_cli_error_payload;
pub(crate) use interface::cli_error::{emit_cli_error, CliError};
pub(crate) use interface::cli_models::{
    AckRisk, ApproveOptions, BrowserActionPayload, BrowserReplayCommandOutput, CheckpointAction,
    CheckpointApprovalPanel, CheckpointCandidate, CheckpointPlaybook, CheckpointSensitiveTarget,
    CliCommand, ClickAdapterOutput, ClickOptions, CompactSnapshotOutput, ExpandAdapterOutput,
    ExpandOptions, ExtractCommandOutput, ExtractOptions, FollowAdapterOutput, FollowOptions,
    MemorySummaryOutput, OutputFormat, PaginateAdapterOutput, PaginateOptions, PaginationDirection,
    PolicyCommandOutput, ReadViewOutput, ReplayCommandOutput, SearchCommandOutput,
    SearchNextCommands, SearchOpenResultCommandOutput, SearchOpenResultOptions,
    SearchOpenTopCommandOutput, SearchOpenTopItem, SearchOpenTopOptions, SearchOptions,
    SecretPrefill, SessionApprovalCommandOutput, SessionApprovalValue,
    SessionCheckpointCommandOutput, SessionCheckpointReport, SessionCloseCommandOutput,
    SessionCloseResultValue, SessionCommandOutput, SessionExtractCommandOutput,
    SessionExtractOptions, SessionFileOptions, SessionPolicyCommandOutput,
    SessionProfileCommandOutput, SessionProfileSetOptions, SessionProfileValue, SessionReadOptions,
    SessionRefreshOptions, SessionSynthesisCommandOutput, SessionSynthesizeOptions,
    SubmitAdapterOutput, SubmitOptions, TargetOptions, TelemetryRecentCommandOutput,
    TelemetryRecentOptions, TelemetrySummaryCommandOutput, TypeAdapterOutput, TypeOptions,
};
pub(crate) use interface::cli_support::{
    current_timestamp, is_fixture_target, repo_root, slot_timestamp, usage,
};
pub(crate) use interface::command_parser::{
    parse_ack_risk, parse_command, parse_output_format, parse_policy_profile, parse_search_engine,
    parse_source_risk,
};

pub(crate) const DEFAULT_OPENED_AT: &str = "2026-03-14T00:00:00+09:00";
pub(crate) const DEFAULT_REQUESTED_TOKENS: usize = 512;
pub(crate) const DEFAULT_SEARCH_TOKENS: usize = 2048;

pub fn run_cli_main(args: Vec<String>) -> i32 {
    interface::cli_entry::run_cli(args)
}

#[cfg(test)]
#[path = "interface/cli_tests.rs"]
mod tests;
