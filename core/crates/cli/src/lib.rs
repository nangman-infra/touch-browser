#![cfg_attr(test, allow(unused_imports))]

pub(crate) mod application;
pub(crate) mod infrastructure;
pub(crate) mod interface;

#[cfg(test)]
pub(crate) use application::browser_session::{BrowserCliSession, PersistedBrowserState};
#[cfg(test)]
pub(crate) use application::search_support::{
    build_search_report, default_search_session_file, derived_search_result_session_file,
};
#[cfg(test)]
pub(crate) use infrastructure::browser_runtime::{
    browser_context_dir_for_session_file, build_browser_cli_session, load_browser_cli_session,
    save_browser_cli_session,
};
#[cfg(test)]
pub(crate) use interface::cli_dispatch::dispatch;
#[cfg(test)]
pub(crate) use interface::cli_entry::{command_usage, preprocess_cli_args};
#[cfg(test)]
pub(crate) use interface::cli_error::build_cli_error_payload;
#[cfg(test)]
pub(crate) use interface::cli_error::CliError;
#[cfg(test)]
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
#[cfg(test)]
pub(crate) use interface::cli_support::{
    current_timestamp, is_fixture_target, repo_root, slot_timestamp, usage,
};
#[cfg(test)]
pub(crate) use interface::command_parser::{
    parse_ack_risk, parse_command, parse_output_format, parse_policy_profile, parse_search_engine,
    parse_source_risk,
};
#[cfg(test)]
pub(crate) use touch_browser_contracts::PolicyProfile;
#[cfg(test)]
pub(crate) use touch_browser_contracts::{
    ActionName, ReplayTranscript, ReplayTranscriptEntry, RiskClass, SearchActionActor,
    SearchEngine, SearchReport, SearchReportStatus, SearchResultItem, SnapshotBlock,
    SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument, SnapshotEvidence,
    SnapshotSource, SourceType, TranscriptKind, TranscriptPayloadType, CONTRACT_VERSION,
};
#[cfg(test)]
pub(crate) use touch_browser_observation::{ObservationCompiler, ObservationInput};

pub(crate) const DEFAULT_OPENED_AT: &str = "2026-03-14T00:00:00+09:00";
pub(crate) const DEFAULT_REQUESTED_TOKENS: usize = 512;
pub(crate) const DEFAULT_SEARCH_TOKENS: usize = 2048;

pub fn run_cli_main(args: Vec<String>) -> i32 {
    interface::cli_entry::run_cli(args)
}

#[cfg(test)]
#[path = "interface/cli_tests.rs"]
mod tests;
