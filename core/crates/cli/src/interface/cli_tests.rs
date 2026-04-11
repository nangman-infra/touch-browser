use std::{
    fs,
    io::Cursor,
    net::TcpListener,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::{json, Value};
use tiny_http::{Header, Response as TinyResponse, Server, StatusCode};
use touch_browser_contracts::{
    ActionName, ReplayTranscript, ReplayTranscriptEntry, RiskClass, SearchReport, SearchResultItem,
    SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument,
    SnapshotEvidence, SnapshotSource, SourceType, TranscriptKind, TranscriptPayloadType,
    CONTRACT_VERSION,
};

use crate::{
    application::search_support::{
        build_search_report, default_search_session_file, derived_search_result_session_file,
    },
    browser_context_dir_for_session_file, build_browser_cli_session, build_cli_error_payload,
    command_usage, dispatch, load_browser_cli_session, parse_command, preprocess_cli_args,
    repo_root, save_browser_cli_session, AckRisk, ApproveOptions, BrowserCliSession, CliCommand,
    CliError, ClickOptions, ExpandOptions, ExtractOptions, FollowOptions, ObservationCompiler,
    ObservationInput, OutputFormat, PaginateOptions, PaginationDirection, PersistedBrowserState,
    PolicyProfile, ReadViewOutput, SearchActionActor, SearchEngine, SearchOpenResultOptions,
    SearchOpenTopOptions, SearchOptions, SearchReportStatus, SessionExtractOptions,
    SessionFileOptions, SessionProfileSetOptions, SessionReadOptions, SessionRefreshOptions,
    SessionSynthesizeOptions, SubmitOptions, TargetOptions, TelemetryRecentOptions, TypeOptions,
    UninstallOptions, UpdateOptions, DEFAULT_OPENED_AT, DEFAULT_REQUESTED_TOKENS,
    DEFAULT_SEARCH_TOKENS,
};

mod cli_tests_browser_sessions;
mod cli_tests_parsing;
mod cli_tests_read_search;
mod cli_tests_support;

use cli_tests_support::*;
