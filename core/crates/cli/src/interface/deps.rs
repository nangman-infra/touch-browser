#![allow(unused_imports)]

pub(crate) use super::cli_dispatch::{dispatch, run_serve};
#[cfg(test)]
pub(crate) use super::cli_error::build_cli_error_payload;
pub(crate) use super::cli_error::{emit_cli_error, CliError};
pub(crate) use super::cli_models::{
    AckRisk, ApproveOptions, BrowserReplayCommandOutput, CliCommand, ClickOptions, ExpandOptions,
    ExtractOptions, FollowOptions, OutputFormat, PaginateOptions, PaginationDirection,
    SearchOpenResultOptions, SearchOpenTopOptions, SearchOptions, SecretPrefill,
    SessionExtractOptions, SessionFileOptions, SessionProfileSetOptions, SessionReadOptions,
    SessionRefreshOptions, SessionSynthesizeOptions, SubmitOptions, TargetOptions,
    TelemetryRecentOptions, TypeOptions, UninstallOptions, UpdateOptions,
};
pub(crate) use super::cli_support::{current_timestamp, repo_root, slot_timestamp, usage};
pub(crate) use super::command_parser::{
    parse_ack_risk, parse_command, parse_output_format, parse_policy_profile, parse_search_engine,
    parse_source_risk,
};
pub(crate) use crate::application;
pub(crate) use crate::application::browser_session::{BrowserCliSession, PersistedBrowserState};
pub(crate) use crate::application::policy_support::{
    approved_risk_labels, merge_ack_risks, policy_profile_label, promoted_policy_profile_for_risks,
};
pub(crate) use crate::application::presentation_support::render_session_synthesis_markdown;
pub(crate) use crate::infrastructure;
pub(crate) use crate::infrastructure::browser_runtime::{
    browser_context_dir_for_session_file, load_browser_cli_session, save_browser_cli_session,
};
pub(crate) use crate::infrastructure::telemetry::{
    log_telemetry_error, log_telemetry_success, telemetry_surface_label,
};
pub(crate) use crate::{DEFAULT_OPENED_AT, DEFAULT_REQUESTED_TOKENS, DEFAULT_SEARCH_TOKENS};
pub(crate) use touch_browser_contracts::{
    EvidenceCitation, PolicyProfile, RiskClass, SearchEngine, SearchReport, SearchReportStatus,
    SearchResultItem, SessionSynthesisClaim, SessionSynthesisClaimStatus, SessionSynthesisReport,
    SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument,
    SnapshotEvidence, SnapshotSource, SourceRisk, SourceType, CONTRACT_VERSION,
};
pub(crate) use touch_browser_observation::{ObservationCompiler, ObservationInput};
pub(crate) use touch_browser_runtime::{ReadOnlyRuntime, RuntimeError};
