use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use touch_browser_contracts::{
    compact_ref_index, navigation_ref_index, render_compact_snapshot,
    render_main_read_view_markdown, render_navigation_compact_snapshot, render_read_view_markdown,
    render_reading_compact_snapshot, ActionResult, CompactRefIndexEntry, PolicyProfile,
    ReplayTranscript, SearchEngine, SessionState, SessionSynthesisReport, SnapshotDocument,
    SourceRisk,
};
use touch_browser_memory::MemorySessionSummary;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliCommand {
    Search(SearchOptions),
    SearchOpenResult(SearchOpenResultOptions),
    SearchOpenTop(SearchOpenTopOptions),
    Open(TargetOptions),
    Snapshot(TargetOptions),
    CompactView(TargetOptions),
    ReadView(TargetOptions),
    Extract(ExtractOptions),
    Policy(TargetOptions),
    SessionSnapshot(SessionFileOptions),
    SessionCompact(SessionFileOptions),
    SessionRead(SessionReadOptions),
    SessionRefresh(SessionRefreshOptions),
    SessionExtract(SessionExtractOptions),
    SessionCheckpoint(SessionFileOptions),
    SessionPolicy(SessionFileOptions),
    SessionProfile(SessionFileOptions),
    SetProfile(SessionProfileSetOptions),
    SessionSynthesize(SessionSynthesizeOptions),
    Approve(ApproveOptions),
    Follow(FollowOptions),
    Click(ClickOptions),
    Type(TypeOptions),
    Submit(SubmitOptions),
    Paginate(PaginateOptions),
    Expand(ExpandOptions),
    BrowserReplay(SessionFileOptions),
    SessionClose(SessionFileOptions),
    TelemetrySummary,
    TelemetryRecent(TelemetryRecentOptions),
    Replay { scenario: String },
    MemorySummary { steps: usize },
    Serve,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TargetOptions {
    pub(crate) target: String,
    pub(crate) budget: usize,
    pub(crate) source_risk: Option<SourceRisk>,
    pub(crate) source_label: Option<String>,
    pub(crate) allowlisted_domains: Vec<String>,
    pub(crate) browser: bool,
    pub(crate) headed: bool,
    pub(crate) main_only: bool,
    pub(crate) session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchOptions {
    pub(crate) query: String,
    pub(crate) engine: SearchEngine,
    pub(crate) budget: usize,
    pub(crate) headed: bool,
    pub(crate) profile_dir: Option<PathBuf>,
    pub(crate) session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchOpenResultOptions {
    pub(crate) engine: SearchEngine,
    pub(crate) session_file: Option<PathBuf>,
    pub(crate) rank: usize,
    pub(crate) headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchOpenTopOptions {
    pub(crate) engine: SearchEngine,
    pub(crate) session_file: Option<PathBuf>,
    pub(crate) limit: usize,
    pub(crate) headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExtractOptions {
    pub(crate) target: String,
    pub(crate) budget: usize,
    pub(crate) source_risk: Option<SourceRisk>,
    pub(crate) source_label: Option<String>,
    pub(crate) allowlisted_domains: Vec<String>,
    pub(crate) browser: bool,
    pub(crate) headed: bool,
    pub(crate) session_file: Option<PathBuf>,
    pub(crate) claims: Vec<String>,
    pub(crate) verifier_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionFileOptions {
    pub(crate) session_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionReadOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) main_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionExtractOptions {
    pub(crate) session_file: Option<PathBuf>,
    pub(crate) claims: Vec<String>,
    pub(crate) verifier_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionSynthesizeOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) note_limit: usize,
    pub(crate) format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum OutputFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionProfileSetOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) profile: PolicyProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TelemetryRecentOptions {
    pub(crate) limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionRefreshOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) headed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AckRisk {
    Challenge,
    Mfa,
    Auth,
    HighRiskWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApproveOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) ack_risks: Vec<AckRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FollowOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) target_ref: String,
    pub(crate) headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ClickOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) target_ref: String,
    pub(crate) headed: bool,
    pub(crate) ack_risks: Vec<AckRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TypeOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) target_ref: String,
    pub(crate) value: String,
    pub(crate) headed: bool,
    pub(crate) sensitive: bool,
    pub(crate) ack_risks: Vec<AckRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubmitOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) target_ref: String,
    pub(crate) headed: bool,
    pub(crate) ack_risks: Vec<AckRisk>,
    pub(crate) extra_prefill: Vec<SecretPrefill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PaginationDirection {
    Next,
    Prev,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PaginateOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) direction: PaginationDirection,
    pub(crate) headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ExpandOptions {
    pub(crate) session_file: PathBuf,
    pub(crate) target_ref: String,
    pub(crate) headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SecretPrefill {
    pub(crate) target_ref: String,
    pub(crate) value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtractCommandOutput {
    pub(crate) open: ActionResult,
    pub(crate) extract: ActionResult,
    pub(crate) session_state: SessionState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PolicyCommandOutput {
    pub(crate) policy: touch_browser_contracts::PolicyReport,
    pub(crate) session_state: SessionState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReplayCommandOutput {
    pub(crate) session_state: SessionState,
    pub(crate) replay_transcript: ReplayTranscript,
    pub(crate) snapshot_count: usize,
    pub(crate) evidence_report_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MemorySummaryOutput {
    pub(crate) requested_actions: usize,
    pub(crate) action_count: usize,
    pub(crate) session_state: SessionState,
    pub(crate) memory_summary: MemorySessionSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionCommandOutput {
    pub(crate) action: ActionResult,
    pub(crate) result: ActionResult,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionExtractCommandOutput {
    pub(crate) extract: ActionResult,
    pub(crate) result: ActionResult,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionPolicyCommandOutput {
    pub(crate) policy: touch_browser_contracts::PolicyReport,
    pub(crate) result: touch_browser_contracts::PolicyReport,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionSynthesisCommandOutput {
    pub(crate) report: SessionSynthesisReport,
    pub(crate) result: SessionSynthesisReport,
    pub(crate) format: OutputFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) markdown: Option<String>,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionCloseCommandOutput {
    pub(crate) session_file: String,
    pub(crate) removed: bool,
    pub(crate) result: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CompactSnapshotOutput {
    pub(crate) source_url: String,
    pub(crate) compact_text: String,
    pub(crate) reading_compact_text: String,
    pub(crate) navigation_compact_text: String,
    pub(crate) line_count: usize,
    pub(crate) char_count: usize,
    pub(crate) approx_tokens: usize,
    pub(crate) ref_index: Vec<CompactRefIndexEntry>,
    pub(crate) navigation_ref_index: Vec<CompactRefIndexEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_state: Option<SessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_file: Option<String>,
}

impl CompactSnapshotOutput {
    pub(crate) fn new(
        snapshot: &SnapshotDocument,
        session_state: Option<SessionState>,
        session_file: Option<String>,
    ) -> Self {
        let compact_text = render_compact_snapshot(snapshot);
        let reading_compact_text = render_reading_compact_snapshot(snapshot);
        let navigation_compact_text = render_navigation_compact_snapshot(snapshot);
        let line_count = compact_text.lines().count();
        let char_count = compact_text.chars().count();
        let approx_tokens = char_count.div_ceil(4).max(1);
        let ref_index = compact_ref_index(snapshot);
        let navigation_ref_index = navigation_ref_index(snapshot);

        Self {
            source_url: snapshot.source.source_url.clone(),
            compact_text,
            reading_compact_text,
            navigation_compact_text,
            line_count,
            char_count,
            approx_tokens,
            ref_index,
            navigation_ref_index,
            session_state,
            session_file,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ReadViewOutput {
    pub(crate) source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) source_title: Option<String>,
    pub(crate) markdown_text: String,
    pub(crate) main_only: bool,
    pub(crate) line_count: usize,
    pub(crate) char_count: usize,
    pub(crate) approx_tokens: usize,
    pub(crate) ref_index: Vec<CompactRefIndexEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_state: Option<SessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) session_file: Option<String>,
}

impl ReadViewOutput {
    pub(crate) fn new(
        snapshot: &SnapshotDocument,
        session_state: Option<SessionState>,
        session_file: Option<String>,
        main_only: bool,
    ) -> Self {
        let markdown_text = if main_only {
            let preferred_markdown = render_main_read_view_markdown(snapshot);
            if preferred_markdown.is_empty() {
                render_read_view_markdown(snapshot)
            } else {
                preferred_markdown
            }
        } else {
            render_read_view_markdown(snapshot)
        };
        let line_count = markdown_text.lines().count();
        let char_count = markdown_text.chars().count();
        let approx_tokens = char_count.div_ceil(4).max(1);
        let ref_index = compact_ref_index(snapshot);

        Self {
            source_url: snapshot.source.source_url.clone(),
            source_title: snapshot.source.title.clone(),
            markdown_text,
            main_only,
            line_count,
            char_count,
            approx_tokens,
            ref_index,
            session_state,
            session_file,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserReplayCommandOutput {
    pub(crate) replayed_actions: usize,
    pub(crate) compact_text: String,
    pub(crate) session_state: SessionState,
}
