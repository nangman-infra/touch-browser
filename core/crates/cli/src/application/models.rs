use std::path::PathBuf;

use super::presentation_support::{
    assess_main_read_view_quality, compact_ref_index, navigation_ref_index,
    render_compact_snapshot, render_main_read_view_markdown, render_navigation_compact_snapshot,
    render_read_view_markdown, render_reading_compact_snapshot,
};
use serde::{Deserialize, Serialize};
use touch_browser_contracts::{
    ActionResult, CaptureDiagnostics, CompactRefIndexEntry, PolicyProfile, PolicyReport,
    ReplayTranscript, SearchEngine, SearchReport, SearchResultItem, SessionState,
    SessionSynthesisReport, SnapshotDocument, SourceRisk,
};
use touch_browser_memory::MemorySessionSummary;
use touch_browser_storage_sqlite::{PilotTelemetryEvent, PilotTelemetrySummary};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CliCommand {
    Capabilities,
    Search(SearchOptions),
    SearchOpenResult(SearchOpenResultOptions),
    SearchOpenTop(SearchOpenTopOptions),
    Mcp,
    Update(UpdateOptions),
    Uninstall(UninstallOptions),
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
    pub(crate) engine_explicit: bool,
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
    pub(crate) prefer_official: bool,
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
pub(crate) struct UpdateOptions {
    pub(crate) check: bool,
    pub(crate) version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UninstallOptions {
    pub(crate) purge_data: bool,
    pub(crate) purge_all: bool,
    pub(crate) yes: bool,
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
    pub(crate) engine: Option<SearchEngine>,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchCommandOutput {
    pub(crate) query: String,
    pub(crate) engine: SearchEngine,
    pub(crate) search_url: String,
    pub(crate) result_count: usize,
    pub(crate) search: SearchReport,
    pub(crate) result: SearchReport,
    pub(crate) browser_context_dir: Option<String>,
    pub(crate) browser_profile_dir: Option<String>,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchOpenResultCommandOutput {
    pub(crate) selected_result: SearchResultItem,
    pub(crate) requested_rank: usize,
    pub(crate) selection_strategy: String,
    pub(crate) result: ActionResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostics: Option<CaptureDiagnostics>,
    pub(crate) session_file: String,
    pub(crate) next_commands: SearchNextCommands,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchNextCommands {
    pub(crate) session_extract: String,
    pub(crate) session_read: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchOpenTopCommandOutput {
    pub(crate) search_session_file: String,
    pub(crate) opened_count: usize,
    pub(crate) opened: Vec<SearchOpenTopItem>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SearchOpenTopItem {
    pub(crate) rank: usize,
    pub(crate) selected_result: SearchResultItem,
    pub(crate) session_file: String,
    pub(crate) result: ActionResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostics: Option<CaptureDiagnostics>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateCommandOutput {
    pub(crate) current_version: String,
    pub(crate) target_version: String,
    pub(crate) update_available: bool,
    pub(crate) checked_only: bool,
    pub(crate) installed: bool,
    pub(crate) release_url: String,
    pub(crate) asset_name: String,
    pub(crate) command_link: String,
    pub(crate) managed_bundle_root: String,
    pub(crate) result: UpdateResultValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UpdateResultValue {
    pub(crate) current_version: String,
    pub(crate) target_version: String,
    pub(crate) update_available: bool,
    pub(crate) checked_only: bool,
    pub(crate) installed: bool,
    pub(crate) release_url: String,
    pub(crate) asset_name: String,
    pub(crate) command_link: String,
    pub(crate) managed_bundle_root: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UninstallCommandOutput {
    pub(crate) removed_paths: Vec<String>,
    pub(crate) purged_data: bool,
    pub(crate) purged_all: bool,
    pub(crate) result: UninstallResultValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UninstallResultValue {
    pub(crate) removed_paths: Vec<String>,
    pub(crate) purged_data: bool,
    pub(crate) purged_all: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExtractCommandOutput {
    pub(crate) open: ActionResult,
    pub(crate) extract: ActionResult,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostics: Option<CaptureDiagnostics>,
    pub(crate) session_state: SessionState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PolicyCommandOutput {
    pub(crate) policy: PolicyReport,
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
    pub(crate) policy: PolicyReport,
    pub(crate) result: PolicyReport,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionProfileCommandOutput {
    pub(crate) policy_profile: String,
    pub(crate) result: SessionProfileValue,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionProfileValue {
    pub(crate) policy_profile: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionApprovalCommandOutput {
    pub(crate) approved_risks: Vec<String>,
    pub(crate) policy_profile: String,
    pub(crate) result: SessionApprovalValue,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionApprovalValue {
    pub(crate) approved_risks: Vec<String>,
    pub(crate) policy_profile: String,
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
    pub(crate) result: SessionCloseResultValue,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionCloseResultValue {
    pub(crate) session_file: String,
    pub(crate) removed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionCheckpointCommandOutput {
    pub(crate) checkpoint: SessionCheckpointReport,
    pub(crate) result: SessionCheckpointReport,
    pub(crate) policy: PolicyReport,
    pub(crate) session_state: SessionState,
    pub(crate) session_file: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SessionCheckpointReport {
    pub(crate) provider_hints: Vec<String>,
    pub(crate) required_ack_risks: Vec<String>,
    pub(crate) approved_risks: Vec<String>,
    pub(crate) active_policy_profile: String,
    pub(crate) recommended_policy_profile: String,
    pub(crate) approval_panel: CheckpointApprovalPanel,
    pub(crate) playbook: CheckpointPlaybook,
    pub(crate) candidates: Vec<CheckpointCandidate>,
    pub(crate) requires_headed_supervision: bool,
    pub(crate) source_url: String,
    pub(crate) source_title: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointApprovalPanel {
    pub(crate) title: String,
    pub(crate) severity: String,
    pub(crate) provider: String,
    pub(crate) active_policy_profile: String,
    pub(crate) recommended_policy_profile: String,
    pub(crate) required_ack_risks: Vec<String>,
    pub(crate) approved_risks: Vec<String>,
    pub(crate) actions: Vec<CheckpointAction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointAction {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) command: String,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) required_ack_risks: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointPlaybook {
    pub(crate) provider: String,
    pub(crate) recommended_policy_profile: String,
    pub(crate) required_ack_risks: Vec<String>,
    pub(crate) approved_risks: Vec<String>,
    pub(crate) steps: Vec<String>,
    pub(crate) sensitive_targets: Vec<CheckpointSensitiveTarget>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointSensitiveTarget {
    pub(crate) r#ref: String,
    pub(crate) text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CheckpointCandidate {
    pub(crate) kind: touch_browser_contracts::SnapshotBlockKind,
    pub(crate) r#ref: String,
    pub(crate) text: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetrySummaryCommandOutput {
    pub(crate) summary: PilotTelemetrySummary,
    pub(crate) result: PilotTelemetrySummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TelemetryRecentCommandOutput {
    pub(crate) limit: usize,
    pub(crate) events: Vec<PilotTelemetryEvent>,
    pub(crate) result: Vec<PilotTelemetryEvent>,
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
pub(crate) struct ReadViewMainContentMetrics {
    pub(crate) body_ratio: f64,
    pub(crate) heading_density: f64,
    pub(crate) nav_ratio: f64,
    pub(crate) kept_block_count: usize,
    pub(crate) candidate_block_count: usize,
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
    pub(crate) main_content_quality: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) main_content_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) main_content_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) main_content_metrics: Option<ReadViewMainContentMetrics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) diagnostics: Option<CaptureDiagnostics>,
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
        let preferred_markdown = render_main_read_view_markdown(snapshot);
        let markdown_text = if main_only {
            if preferred_markdown.is_empty() {
                render_read_view_markdown(snapshot)
            } else {
                preferred_markdown.clone()
            }
        } else {
            render_read_view_markdown(snapshot)
        };
        let quality_markdown = if preferred_markdown.is_empty() {
            markdown_text.clone()
        } else {
            preferred_markdown
        };
        let line_count = markdown_text.lines().count();
        let char_count = markdown_text.chars().count();
        let approx_tokens = char_count.div_ceil(4).max(1);
        let ref_index = compact_ref_index(snapshot);
        let (main_content_quality, main_content_reason, main_content_hint, main_content_metrics) =
            assess_main_read_view_quality(snapshot, &quality_markdown)
                .map(|(quality, reason, hint, metrics)| {
                    (
                        Some(quality.as_str().to_string()),
                        Some(reason.as_str().to_string()),
                        Some(hint),
                        Some(ReadViewMainContentMetrics {
                            body_ratio: metrics.body_ratio,
                            heading_density: metrics.heading_density,
                            nav_ratio: metrics.nav_ratio,
                            kept_block_count: metrics.kept_block_count,
                            candidate_block_count: metrics.candidate_block_count,
                        }),
                    )
                })
                .unwrap_or((None, None, None, None));

        Self {
            source_url: snapshot.source.source_url.clone(),
            source_title: snapshot.source.title.clone(),
            markdown_text,
            main_only,
            line_count,
            char_count,
            approx_tokens,
            ref_index,
            main_content_quality,
            main_content_reason,
            main_content_hint,
            main_content_metrics,
            diagnostics: None,
            session_state,
            session_file,
        }
    }

    pub(crate) fn with_diagnostics(mut self, diagnostics: CaptureDiagnostics) -> Self {
        self.diagnostics = Some(diagnostics);
        self
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserReplayCommandOutput {
    pub(crate) replayed_actions: usize,
    pub(crate) compact_text: String,
    pub(crate) session_state: SessionState,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserActionPayload<T>
where
    T: Serialize,
{
    pub(crate) snapshot: SnapshotDocument,
    pub(crate) adapter: T,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FollowAdapterOutput {
    pub(crate) followed_ref: String,
    pub(crate) target_text: String,
    pub(crate) target_href: Option<String>,
    pub(crate) clicked_text: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) final_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DownloadEvidenceOutput {
    pub(crate) completed: bool,
    pub(crate) suggested_filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) byte_length: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) failure: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ClickAdapterOutput {
    pub(crate) clicked_ref: String,
    pub(crate) target_text: String,
    pub(crate) target_href: Option<String>,
    pub(crate) clicked_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) download: Option<DownloadEvidenceOutput>,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) final_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TypeAdapterOutput {
    pub(crate) typed_ref: String,
    pub(crate) target_text: String,
    pub(crate) typed_length: usize,
    pub(crate) sensitive: bool,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) final_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubmitAdapterOutput {
    pub(crate) submitted_ref: String,
    pub(crate) target_text: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) final_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PaginateAdapterOutput {
    pub(crate) page: usize,
    pub(crate) clicked_text: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) final_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExpandAdapterOutput {
    pub(crate) expanded_ref: String,
    pub(crate) target_text: String,
    pub(crate) clicked_text: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) final_url: String,
}
