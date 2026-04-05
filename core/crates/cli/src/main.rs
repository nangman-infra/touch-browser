use std::{
    collections::BTreeSet,
    env, fs,
    path::{Path, PathBuf},
};

use kuchiki::{parse_html, traits::*, NodeRef};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use touch_browser_acquisition::{AcquisitionConfig, AcquisitionEngine, AcquisitionError};
use touch_browser_action_vm::ReadOnlyActionVm;
use touch_browser_contracts::{
    compact_ref_index, navigation_ref_index, render_compact_snapshot,
    render_main_read_view_markdown, render_navigation_compact_snapshot, render_read_view_markdown,
    render_reading_compact_snapshot, ActionCommand, ActionFailureKind, ActionName, ActionResult,
    ActionStatus, CompactRefIndexEntry, EvidenceCitation, EvidenceClaimOutcome,
    EvidenceClaimVerdict, EvidenceReport, EvidenceVerificationOutcome, EvidenceVerificationReport,
    EvidenceVerificationVerdict, PolicyProfile, PolicyReport, ReplayTranscript, RiskClass,
    SearchActionActor, SearchActionHint, SearchEngine, SearchReport, SearchReportStatus,
    SearchResultItem, SessionMode, SessionState, SessionSynthesisClaim,
    SessionSynthesisClaimStatus, SessionSynthesisReport, SnapshotBlock, SnapshotBlockKind,
    SnapshotBlockRole, SnapshotDocument, SourceRisk, SourceType, UnsupportedClaimReason,
    CONTRACT_VERSION,
};
use touch_browser_memory::{plan_memory_turn, summarize_turns, MemorySessionSummary};
use touch_browser_observation::{
    recommend_requested_tokens, ObservationCompiler, ObservationInput,
};
use touch_browser_policy::PolicyKernel;
use touch_browser_runtime::{
    CatalogDocument, ClaimInput, FixtureCatalog, ReadOnlyRuntime, ReadOnlySession, RuntimeError,
};
use touch_browser_storage_sqlite::TelemetryError;
use url::{form_urlencoded, Url};

mod application;
mod infrastructure;
mod interface;

pub(crate) use application::session_reporting::{
    render_session_synthesis_markdown, verify_action_result_if_requested,
};
pub(crate) use infrastructure::{browser_runtime::*, telemetry::*};
pub(crate) use interface::serve_runtime::ServeDaemonState;

const DEFAULT_OPENED_AT: &str = "2026-03-14T00:00:00+09:00";
const DEFAULT_REQUESTED_TOKENS: usize = 512;
const DEFAULT_SEARCH_TOKENS: usize = 2048;

fn main() {
    let args = env::args().skip(1).collect::<Vec<_>>();
    let operation = args
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let command = match parse_command(&args) {
        Ok(command) => command,
        Err(error) => {
            let _ = log_telemetry_error(
                &telemetry_surface_label("cli"),
                &operation,
                &error.to_string(),
                None,
                &Value::Null,
            );
            eprintln!("{error}");
            std::process::exit(1);
        }
    };

    if matches!(command, CliCommand::Serve) {
        if let Err(error) = handle_serve() {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return;
    }

    let stdout_mode = stdout_mode_for_command(&command);

    match dispatch(command) {
        Ok(output) => {
            let _ = log_telemetry_success(
                &telemetry_surface_label("cli"),
                &operation,
                &output,
                &Value::Null,
            );
            match stdout_mode {
                CliStdoutMode::Json => println!(
                    "{}",
                    serde_json::to_string_pretty(&output).expect("cli output should serialize")
                ),
                CliStdoutMode::ReadMarkdown => {
                    println!("{}", required_output_string(&output, "markdownText"))
                }
                CliStdoutMode::SynthesisMarkdown => {
                    println!("{}", required_output_string(&output, "markdown"))
                }
            }
        }
        Err(error) => {
            let _ = log_telemetry_error(
                &telemetry_surface_label("cli"),
                &operation,
                &error.to_string(),
                None,
                &Value::Null,
            );
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliStdoutMode {
    Json,
    ReadMarkdown,
    SynthesisMarkdown,
}

fn stdout_mode_for_command(command: &CliCommand) -> CliStdoutMode {
    match command {
        CliCommand::ReadView(_) | CliCommand::SessionRead(_) => CliStdoutMode::ReadMarkdown,
        CliCommand::SessionSynthesize(options) if options.format == OutputFormat::Markdown => {
            CliStdoutMode::SynthesisMarkdown
        }
        _ => CliStdoutMode::Json,
    }
}

fn required_output_string<'a>(output: &'a Value, field: &str) -> &'a str {
    output
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("expected `{field}` string output"))
}

fn dispatch(command: CliCommand) -> Result<Value, CliError> {
    match command {
        CliCommand::Search(options) => handle_search(options),
        CliCommand::SearchOpenResult(options) => handle_search_open_result(options),
        CliCommand::SearchOpenTop(options) => handle_search_open_top(options),
        CliCommand::Open(options) => handle_open(options),
        CliCommand::Snapshot(options) => handle_open(options),
        CliCommand::CompactView(options) => handle_compact_view(options),
        CliCommand::ReadView(options) => handle_read_view(options),
        CliCommand::Extract(options) => handle_extract(options),
        CliCommand::Policy(options) => handle_policy(options),
        CliCommand::SessionSnapshot(options) => handle_session_snapshot(options),
        CliCommand::SessionCompact(options) => handle_session_compact(options),
        CliCommand::SessionRead(options) => handle_session_read(options),
        CliCommand::SessionRefresh(options) => handle_session_refresh(options),
        CliCommand::SessionExtract(options) => handle_session_extract(options),
        CliCommand::SessionCheckpoint(options) => handle_session_checkpoint(options),
        CliCommand::SessionPolicy(options) => handle_session_policy(options),
        CliCommand::SessionProfile(options) => handle_session_profile(options),
        CliCommand::SetProfile(options) => handle_set_profile(options),
        CliCommand::SessionSynthesize(options) => handle_session_synthesize(options),
        CliCommand::Approve(options) => handle_approve(options),
        CliCommand::Follow(options) => handle_follow(options),
        CliCommand::Click(options) => handle_click(options),
        CliCommand::Type(options) => handle_type(options),
        CliCommand::Submit(options) => handle_submit(options),
        CliCommand::Paginate(options) => handle_paginate(options),
        CliCommand::Expand(options) => handle_expand(options),
        CliCommand::BrowserReplay(options) => handle_browser_replay(options),
        CliCommand::SessionClose(options) => handle_session_close(options),
        CliCommand::TelemetrySummary => handle_telemetry_summary(),
        CliCommand::TelemetryRecent(options) => handle_telemetry_recent(options),
        CliCommand::Replay { scenario } => handle_replay(&scenario),
        CliCommand::MemorySummary { steps } => handle_memory_summary(steps),
        CliCommand::Serve => Err(CliError::Usage(
            "serve is handled directly and should not be dispatched.".to_string(),
        )),
    }
}

fn handle_search(options: SearchOptions) -> Result<Value, CliError> {
    application::research_commands::handle_search(options)
}

fn handle_search_open_result(options: SearchOpenResultOptions) -> Result<Value, CliError> {
    application::research_commands::handle_search_open_result(options)
}

fn handle_search_open_top(options: SearchOpenTopOptions) -> Result<Value, CliError> {
    application::research_commands::handle_search_open_top(options)
}

fn build_search_url(engine: SearchEngine, query: &str) -> Result<String, CliError> {
    let base = match engine {
        SearchEngine::Google => "https://www.google.com/search",
        SearchEngine::Brave => "https://search.brave.com/search",
    };
    let mut serializer = form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("q", query);
    let query_string = serializer.finish();
    Ok(format!("{base}?{query_string}"))
}

fn search_engine_slug(engine: SearchEngine) -> &'static str {
    match engine {
        SearchEngine::Google => "google",
        SearchEngine::Brave => "brave",
    }
}

fn default_search_session_file(engine: SearchEngine) -> PathBuf {
    repo_root().join("output/browser-search").join(format!(
        "{}.search-session.json",
        search_engine_slug(engine)
    ))
}

fn resolve_search_session_file(session_file: Option<&PathBuf>, engine: SearchEngine) -> PathBuf {
    session_file
        .cloned()
        .unwrap_or_else(|| default_search_session_file(engine))
}

fn derived_search_result_session_file(search_session_file: &Path, rank: usize) -> PathBuf {
    let parent = search_session_file
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"));
    let stem = search_session_file
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "touch-browser-search".to_string());
    parent.join(format!("{stem}.rank-{rank}.json"))
}

fn is_search_results_target(target: &str) -> bool {
    let Ok(url) = Url::parse(target) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let path = url.path();
    (is_google_host(host) && path == "/search") || (is_brave_host(host) && path == "/search")
}

fn build_search_report(
    engine: SearchEngine,
    query: &str,
    search_url: &str,
    snapshot: &SnapshotDocument,
    html: &str,
    final_url: &str,
    generated_at: &str,
) -> SearchReport {
    let mut results = extract_search_results_from_snapshot(engine, query, snapshot);
    merge_search_results(
        &mut results,
        extract_search_results_from_html(engine, query, final_url, html),
    );
    let (status, status_detail) = search_report_status(engine, snapshot, final_url, &results);
    for result in &mut results {
        result.selection_score = Some(round_search_score(selection_score_for_result(
            query, result,
        )));
        result.recommended_surface = Some(recommended_surface_for_result(query, result));
    }

    let mut recommended = results
        .iter()
        .map(|result| {
            (
                result.rank,
                result.selection_score.unwrap_or(0.0),
                result.official_likely,
            )
        })
        .collect::<Vec<_>>();
    recommended.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.0.cmp(&right.0))
    });
    let recommended_result_ranks = recommended
        .into_iter()
        .take(3)
        .map(|entry| entry.0)
        .collect::<Vec<_>>();
    let next_action_hints = search_action_hints(
        query,
        &results,
        &recommended_result_ranks,
        status,
        status_detail.as_deref(),
    );

    SearchReport {
        version: CONTRACT_VERSION.to_string(),
        generated_at: generated_at.to_string(),
        engine,
        query: query.to_string(),
        search_url: search_url.to_string(),
        final_url: final_url.to_string(),
        status,
        status_detail,
        result_count: results.len(),
        results,
        recommended_result_ranks,
        next_action_hints,
    }
}

fn extract_search_results_from_snapshot(
    engine: SearchEngine,
    query: &str,
    snapshot: &SnapshotDocument,
) -> Vec<SearchResultItem> {
    let mut results = Vec::new();
    let mut seen_urls = BTreeSet::new();

    for (index, block) in snapshot.blocks.iter().enumerate() {
        if block.kind != SnapshotBlockKind::Link {
            continue;
        }
        if matches!(
            block.role,
            SnapshotBlockRole::PrimaryNav
                | SnapshotBlockRole::SecondaryNav
                | SnapshotBlockRole::Cta
                | SnapshotBlockRole::FormControl
        ) {
            continue;
        }
        let Some(raw_href) = block.attributes.get("href").and_then(Value::as_str) else {
            continue;
        };
        let Some(url) = normalize_search_result_url(engine, &snapshot.source.source_url, raw_href)
        else {
            continue;
        };
        if !seen_urls.insert(url.clone()) {
            continue;
        }
        let title = block.text.trim().to_string();
        if title.len() < 6 {
            continue;
        }
        let snippet = collect_search_result_snippet(&snapshot.blocks, index);
        let domain = url_domain(&url);

        results.push(SearchResultItem {
            rank: results.len() + 1,
            title,
            url,
            domain: domain.clone(),
            snippet,
            stable_ref: Some(block.stable_ref.clone()),
            official_likely: official_likely(query, &domain),
            selection_score: None,
            recommended_surface: None,
        });
    }

    results
}

fn extract_search_results_from_html(
    engine: SearchEngine,
    query: &str,
    base_url: &str,
    html: &str,
) -> Vec<SearchResultItem> {
    let document = parse_html().one(html.to_string());
    let mut results = Vec::new();
    let mut seen_urls = BTreeSet::new();

    let Ok(anchors) = document.select("a") else {
        return results;
    };

    for anchor in anchors {
        let Some(attributes) = anchor.attributes.borrow().get("href").map(str::to_string) else {
            continue;
        };
        let Some(url) = normalize_search_result_url(engine, base_url, &attributes) else {
            continue;
        };
        if !seen_urls.insert(url.clone()) {
            continue;
        }

        let title = collapse_whitespace(&anchor.text_contents());
        if title.len() < 6 {
            continue;
        }
        if looks_like_search_nav_link(&title) {
            continue;
        }
        let domain = url_domain(&url);
        let snippet = search_result_snippet_from_anchor(&anchor.as_node(), &title);

        results.push(SearchResultItem {
            rank: results.len() + 1,
            title,
            url,
            domain: domain.clone(),
            snippet,
            stable_ref: None,
            official_likely: official_likely(query, &domain),
            selection_score: None,
            recommended_surface: None,
        });
    }

    results
}

fn merge_search_results(results: &mut Vec<SearchResultItem>, additional: Vec<SearchResultItem>) {
    let mut seen = results
        .iter()
        .map(|result| result.url.clone())
        .collect::<BTreeSet<_>>();
    for candidate in additional {
        if !seen.insert(candidate.url.clone()) {
            continue;
        }
        let mut candidate = candidate;
        candidate.rank = results.len() + 1;
        results.push(candidate);
    }
}

fn search_report_status(
    engine: SearchEngine,
    snapshot: &SnapshotDocument,
    final_url: &str,
    results: &[SearchResultItem],
) -> (SearchReportStatus, Option<String>) {
    if let Some(detail) = detect_search_challenge(engine, snapshot, final_url) {
        return (SearchReportStatus::Challenge, Some(detail));
    }
    if results.is_empty() {
        return (
            SearchReportStatus::NoResults,
            Some("No search results were structured from the current result page.".to_string()),
        );
    }
    (SearchReportStatus::Ready, None)
}

fn detect_search_challenge(
    engine: SearchEngine,
    snapshot: &SnapshotDocument,
    final_url: &str,
) -> Option<String> {
    let final_lowered = final_url.to_ascii_lowercase();
    let title_lowered = snapshot
        .source
        .title
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let body_text = snapshot
        .blocks
        .iter()
        .take(24)
        .map(|block| block.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase();

    let signals = [
        "captcha",
        "recaptcha",
        "confirm you're not a robot",
        "i'm not a robot",
        "비정상적인 트래픽",
        "로봇이 아닙니다",
        "drag the slider",
        "human checkpoint",
    ];
    if signals.iter().any(|signal| {
        final_lowered.contains(signal)
            || title_lowered.contains(signal)
            || body_text.contains(signal)
    }) {
        return Some(match engine {
            SearchEngine::Google => "Google returned a bot-check or reCAPTCHA page instead of a normal result list. Re-run in headed mode, clear the challenge manually, then search again.".to_string(),
            SearchEngine::Brave => "Brave returned a CAPTCHA or slider challenge instead of a normal result list. Re-run in headed mode, clear the challenge manually, then search again.".to_string(),
        });
    }

    match engine {
        SearchEngine::Google if final_lowered.contains("/sorry/") => Some(
            "Google returned a traffic verification page instead of a normal result list. Re-run in headed mode, clear the challenge manually, then search again.".to_string(),
        ),
        _ => None,
    }
}

fn search_result_snippet_from_anchor(anchor: &NodeRef, title: &str) -> Option<String> {
    let mut candidate = anchor.parent();
    while let Some(node) = candidate {
        let text = collapse_whitespace(&node.text_contents());
        if text.len() > title.len() + 24 {
            let snippet = text.replacen(title, "", 1);
            let snippet = collapse_whitespace(&snippet);
            if snippet.len() >= 20 {
                return Some(truncate_plain_text(&snippet, 220));
            }
        }
        candidate = node.parent();
    }
    None
}

fn looks_like_search_nav_link(text: &str) -> bool {
    let lowered = text.to_ascii_lowercase();
    [
        "images",
        "news",
        "videos",
        "maps",
        "shopping",
        "more",
        "settings",
        "tools",
        "sign in",
        "feedback",
        "help",
        "다음",
        "이전",
        "도움말",
        "설정",
        "이미지",
        "뉴스",
        "동영상",
    ]
    .iter()
    .any(|keyword| lowered == *keyword || lowered.starts_with(&format!("{keyword} ")))
}

fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn collect_search_result_snippet(blocks: &[SnapshotBlock], start_index: usize) -> Option<String> {
    let mut parts = Vec::new();
    for block in blocks.iter().skip(start_index + 1).take(6) {
        if matches!(
            block.kind,
            SnapshotBlockKind::Heading | SnapshotBlockKind::Link
        ) {
            break;
        }
        if !matches!(
            block.role,
            SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
        ) {
            continue;
        }
        if !matches!(
            block.kind,
            SnapshotBlockKind::Text | SnapshotBlockKind::List | SnapshotBlockKind::Metadata
        ) {
            continue;
        }
        let text = block.text.trim();
        if text.is_empty() {
            continue;
        }
        parts.push(text.to_string());
        if parts.join(" ").len() >= 220 {
            break;
        }
    }

    let snippet = parts.join(" ");
    (!snippet.is_empty()).then(|| truncate_plain_text(&snippet, 220))
}

fn normalize_search_result_url(
    engine: SearchEngine,
    base_url: &str,
    raw_href: &str,
) -> Option<String> {
    let base = Url::parse(base_url).ok()?;
    let resolved = base.join(raw_href).or_else(|_| Url::parse(raw_href)).ok()?;
    let host = resolved.host_str()?;

    match engine {
        SearchEngine::Google if is_google_host(host) => {
            if resolved.path() == "/url" || resolved.path() == "/imgres" {
                for key in ["q", "url", "imgurl"] {
                    if let Some(value) = resolved
                        .query_pairs()
                        .find(|(candidate, _)| candidate == key)
                        .map(|(_, value)| value.into_owned())
                    {
                        if value.starts_with("http://") || value.starts_with("https://") {
                            return Some(value);
                        }
                    }
                }
            }
            None
        }
        SearchEngine::Brave if is_brave_host(host) => None,
        _ => matches!(resolved.scheme(), "http" | "https").then(|| resolved.to_string()),
    }
}

fn is_google_host(host: &str) -> bool {
    host == "google.com"
        || host == "www.google.com"
        || host.ends_with(".google.com")
        || host.ends_with(".google.co.kr")
}

fn is_brave_host(host: &str) -> bool {
    host == "search.brave.com" || host.ends_with(".brave.com")
}

fn url_domain(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(ToString::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn search_engine_source_label(engine: SearchEngine) -> &'static str {
    match engine {
        SearchEngine::Google => "Google Search",
        SearchEngine::Brave => "Brave Search",
    }
}

fn official_likely(query: &str, domain: &str) -> bool {
    let lowered_domain = domain.to_ascii_lowercase();
    let query_tokens = search_query_tokens(query);
    lowered_domain.starts_with("docs.")
        || lowered_domain.starts_with("developer.")
        || lowered_domain.contains("developers.")
        || lowered_domain.contains("developer.")
        || lowered_domain.contains("docs.")
        || lowered_domain.ends_with(".gov")
        || lowered_domain.ends_with(".edu")
        || lowered_domain.contains("mdn")
        || query_tokens
            .iter()
            .any(|token| token.len() >= 4 && lowered_domain.contains(token))
}

fn selection_score_for_result(query: &str, result: &SearchResultItem) -> f64 {
    let lowered_title = result.title.to_ascii_lowercase();
    let lowered_url = result.url.to_ascii_lowercase();
    let lowered_snippet = result
        .snippet
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let numeric_intent = query_has_numeric_intent(query);
    let detail_keywords = [
        "limit",
        "limits",
        "quota",
        "quotas",
        "pricing",
        "price",
        "cost",
        "timeout",
        "release",
        "version",
        "versions",
        "reference",
        "api",
        "docs",
    ];
    let overview_keywords = ["overview", "guide", "intro", "introduction", "manual"];

    let mut score = 0.6 / result.rank as f64;
    if result.official_likely {
        score += 0.25;
    }
    if detail_keywords.iter().any(|keyword| {
        lowered_title.contains(keyword)
            || lowered_url.contains(keyword)
            || lowered_snippet.contains(keyword)
    }) {
        score += if numeric_intent { 0.22 } else { 0.10 };
    }
    if overview_keywords.iter().any(|keyword| {
        lowered_title.contains(keyword)
            || lowered_url.contains(keyword)
            || lowered_snippet.contains(keyword)
    }) {
        score += if numeric_intent { 0.04 } else { 0.12 };
    }
    if search_query_tokens(query)
        .iter()
        .any(|token| lowered_title.contains(token) || lowered_url.contains(token))
    {
        score += 0.10;
    }
    score.clamp(0.0, 1.0)
}

fn recommended_surface_for_result(query: &str, result: &SearchResultItem) -> String {
    let lowered = format!(
        "{} {} {}",
        result.title.to_ascii_lowercase(),
        result.url.to_ascii_lowercase(),
        result
            .snippet
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase()
    );
    if query_has_numeric_intent(query)
        || ["limit", "quota", "pricing", "timeout", "release", "version"]
            .iter()
            .any(|keyword| lowered.contains(keyword))
    {
        "extract".to_string()
    } else {
        "read-view".to_string()
    }
}

fn search_action_hints(
    query: &str,
    results: &[SearchResultItem],
    recommended_ranks: &[usize],
    status: SearchReportStatus,
    status_detail: Option<&str>,
) -> Vec<SearchActionHint> {
    if status == SearchReportStatus::Challenge {
        return vec![SearchActionHint {
            action: "complete-challenge".to_string(),
            detail: status_detail.unwrap_or("The search provider returned a challenge page. Re-run in headed mode, clear the challenge manually, then retry search.").to_string(),
            actor: SearchActionActor::Human,
            can_auto_run: false,
            headed_required: true,
            result_ranks: Vec::new(),
        }];
    }

    if results.is_empty() {
        return vec![SearchActionHint {
            action: "refine-search".to_string(),
            detail: status_detail.unwrap_or("No external results were structured from the current search page. Retry with a narrower query or run in headed mode.").to_string(),
            actor: SearchActionActor::Ai,
            can_auto_run: false,
            headed_required: false,
            result_ranks: Vec::new(),
        }];
    }

    let mut hints = vec![SearchActionHint {
        action: "open-top".to_string(),
        detail: "Open the highest-ranked candidate tabs first, then run read-view or extract on the most specific pages.".to_string(),
        actor: SearchActionActor::Ai,
        can_auto_run: true,
        headed_required: false,
        result_ranks: recommended_ranks.to_vec(),
    }];

    let official_ranks = results
        .iter()
        .filter(|result| result.official_likely)
        .map(|result| result.rank)
        .take(3)
        .collect::<Vec<_>>();
    if !official_ranks.is_empty() {
        hints.push(SearchActionHint {
            action: "prefer-official".to_string(),
            detail:
                "Prefer documentation-like or official domains before making an evidence judgment."
                    .to_string(),
            actor: SearchActionActor::Ai,
            can_auto_run: false,
            headed_required: false,
            result_ranks: official_ranks,
        });
    }

    if query_has_numeric_intent(query) {
        hints.push(SearchActionHint {
            action: "extract".to_string(),
            detail: "This query looks numeric or limit-sensitive. Prefer limits, pricing, release-note, or reference pages before answering.".to_string(),
            actor: SearchActionActor::Ai,
            can_auto_run: true,
            headed_required: false,
            result_ranks: recommended_ranks.to_vec(),
        });
    } else {
        hints.push(SearchActionHint {
            action: "read-view".to_string(),
            detail: "Use read-view on the most relevant tabs first, then run extract only after the scope looks right.".to_string(),
            actor: SearchActionActor::Ai,
            can_auto_run: true,
            headed_required: false,
            result_ranks: recommended_ranks.to_vec(),
        });
    }

    hints
}

fn query_has_numeric_intent(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    lowered.chars().any(|character| character.is_ascii_digit())
        || [
            "limit", "limits", "quota", "quotas", "price", "pricing", "cost", "timeout", "version",
            "release", "released", "seconds", "minutes", "hours", "size", "latency", "memory",
            "date", "when",
        ]
        .iter()
        .any(|keyword| lowered.contains(keyword))
}

fn search_query_tokens(text: &str) -> Vec<String> {
    text.to_ascii_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| token.len() >= 3)
        .map(ToString::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn round_search_score(score: f64) -> f64 {
    (score * 100.0).round() / 100.0
}

fn truncate_plain_text(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        return text.to_string();
    }
    text.chars()
        .take(limit.saturating_sub(1))
        .collect::<String>()
        + "…"
}

fn handle_open(options: TargetOptions) -> Result<Value, CliError> {
    application::research_commands::handle_open(options)
}

fn handle_compact_view(options: TargetOptions) -> Result<Value, CliError> {
    application::research_commands::handle_compact_view(options)
}

fn handle_read_view(options: TargetOptions) -> Result<Value, CliError> {
    application::research_commands::handle_read_view(options)
}

fn handle_extract(options: ExtractOptions) -> Result<Value, CliError> {
    application::research_commands::handle_extract(options)
}

fn handle_policy(options: TargetOptions) -> Result<Value, CliError> {
    application::research_commands::handle_policy(options)
}

fn handle_replay(scenario: &str) -> Result<Value, CliError> {
    application::research_commands::handle_replay(scenario)
}

fn handle_memory_summary(steps: usize) -> Result<Value, CliError> {
    application::research_commands::handle_memory_summary(steps)
}

fn handle_session_snapshot(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_snapshot(options)
}

fn handle_session_compact(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_compact(options)
}

fn handle_session_read(options: SessionReadOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_read(options)
}

fn handle_session_refresh(options: SessionRefreshOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_refresh(options)
}

fn handle_session_extract(options: SessionExtractOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_extract(options)
}

fn handle_session_policy(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_policy(options)
}

fn handle_session_profile(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_profile(options)
}

fn handle_set_profile(options: SessionProfileSetOptions) -> Result<Value, CliError> {
    application::session_commands::handle_set_profile(options)
}

fn handle_session_checkpoint(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_checkpoint(options)
}

fn handle_session_synthesize(options: SessionSynthesizeOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_synthesize(options)
}

fn handle_approve(options: ApproveOptions) -> Result<Value, CliError> {
    application::session_commands::handle_approve(options)
}

fn handle_telemetry_summary() -> Result<Value, CliError> {
    application::session_commands::handle_telemetry_summary()
}

fn handle_telemetry_recent(options: TelemetryRecentOptions) -> Result<Value, CliError> {
    application::session_commands::handle_telemetry_recent(options)
}

fn handle_follow(options: FollowOptions) -> Result<Value, CliError> {
    application::browser_session_actions::handle_follow(options)
}

fn handle_click(options: ClickOptions) -> Result<Value, CliError> {
    application::browser_session_actions::handle_click(options)
}

fn handle_type(options: TypeOptions) -> Result<Value, CliError> {
    application::browser_session_actions::handle_type(options)
}

fn handle_submit(options: SubmitOptions) -> Result<Value, CliError> {
    application::browser_session_actions::handle_submit(options)
}

fn handle_paginate(options: PaginateOptions) -> Result<Value, CliError> {
    application::browser_session_actions::handle_paginate(options)
}

fn handle_expand(options: ExpandOptions) -> Result<Value, CliError> {
    application::browser_session_actions::handle_expand(options)
}

fn handle_session_close(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_session_close(options)
}

fn handle_browser_replay(options: SessionFileOptions) -> Result<Value, CliError> {
    application::session_commands::handle_browser_replay(options)
}

fn handle_serve() -> Result<(), CliError> {
    interface::serve_runtime::handle_serve()
}

fn parse_command(args: &[String]) -> Result<CliCommand, CliError> {
    interface::command_parser::parse_command(args)
}

fn parse_search_engine(value: &str) -> Result<SearchEngine, CliError> {
    interface::command_parser::parse_search_engine(value)
}

fn parse_ack_risk(value: &str) -> Result<AckRisk, CliError> {
    interface::command_parser::parse_ack_risk(value)
}

fn parse_policy_profile(value: &str) -> Result<PolicyProfile, CliError> {
    interface::command_parser::parse_policy_profile(value)
}

fn parse_output_format(value: &str) -> Result<OutputFormat, CliError> {
    interface::command_parser::parse_output_format(value)
}

fn parse_source_risk(value: &str) -> Result<SourceRisk, CliError> {
    interface::command_parser::parse_source_risk(value)
}

fn load_fixture_catalog() -> Result<FixtureCatalog, CliError> {
    let mut catalog = FixtureCatalog::default();

    for metadata_path in fixture_metadata_paths()? {
        let metadata: FixtureMetadata = serde_json::from_str(&fs::read_to_string(&metadata_path)?)?;
        let html = fs::read_to_string(repo_root().join(metadata.html_path))?;
        let risk = parse_source_risk(&metadata.risk)?;

        catalog.register(
            CatalogDocument::new(
                metadata.source_uri.clone(),
                html,
                SourceType::Fixture,
                risk,
                Some(metadata.title),
            )
            .with_aliases(default_aliases(&metadata.source_uri)),
        );
    }

    Ok(catalog)
}

fn fixture_metadata_paths() -> Result<Vec<PathBuf>, CliError> {
    let research_root = repo_root().join("fixtures/research");
    let mut paths = Vec::new();

    for category in fs::read_dir(research_root)? {
        let category = category?;
        if !category.file_type()?.is_dir() {
            continue;
        }

        for fixture in fs::read_dir(category.path())? {
            let fixture = fixture?;
            if !fixture.file_type()?.is_dir() {
                continue;
            }

            let metadata_path = fixture.path().join("fixture.json");
            if metadata_path.is_file() {
                paths.push(metadata_path);
            }
        }
    }

    paths.sort();
    Ok(paths)
}

fn default_aliases(source_uri: &str) -> Vec<String> {
    match source_uri {
        "fixture://research/static-docs/getting-started" => {
            vec!["/docs".to_string(), "/getting-started".to_string()]
        }
        "fixture://research/citation-heavy/pricing" => vec!["/pricing".to_string()],
        "fixture://research/navigation/api-reference" => {
            vec!["/api".to_string(), "/api-reference".to_string()]
        }
        _ => Vec::new(),
    }
}

fn current_policy_with_allowlist(
    session: &ReadOnlySession,
    kernel: &PolicyKernel,
    allowlisted_domains: &[String],
) -> Option<PolicyReport> {
    session.current_snapshot_record().map(|record| {
        kernel.evaluate_snapshot_with_allowlist(
            &record.snapshot,
            record.source_risk.clone(),
            allowlisted_domains,
        )
    })
}

fn succeed_action(
    action: ActionName,
    payload_type: &str,
    output: Value,
    message: &str,
    policy: Option<PolicyReport>,
) -> ActionResult {
    ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Succeeded,
        payload_type: payload_type.to_string(),
        output: Some(output),
        policy,
        failure_kind: None,
        message: message.to_string(),
    }
}

fn fail_action(
    action: ActionName,
    failure_kind: ActionFailureKind,
    message: &str,
    policy: Option<PolicyReport>,
) -> ActionResult {
    ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Failed,
        payload_type: "none".to_string(),
        output: None,
        policy,
        failure_kind: Some(failure_kind),
        message: message.to_string(),
    }
}

fn reject_action(
    action: ActionName,
    failure_kind: ActionFailureKind,
    message: &str,
    policy: Option<PolicyReport>,
) -> ActionResult {
    ActionResult {
        version: CONTRACT_VERSION.to_string(),
        action,
        status: ActionStatus::Rejected,
        payload_type: "none".to_string(),
        output: None,
        policy,
        failure_kind: Some(failure_kind),
        message: message.to_string(),
    }
}

fn ack_risk_label(ack_risk: AckRisk) -> &'static str {
    match ack_risk {
        AckRisk::Challenge => "challenge",
        AckRisk::Mfa => "mfa",
        AckRisk::Auth => "auth",
        AckRisk::HighRiskWrite => "high-risk-write",
    }
}

fn approved_risk_labels(approved_risks: &BTreeSet<AckRisk>) -> Vec<String> {
    approved_risks
        .iter()
        .map(|risk| ack_risk_label(*risk).to_string())
        .collect()
}

fn has_ack_risk(
    ack_risks: &[AckRisk],
    approved_risks: &BTreeSet<AckRisk>,
    expected: AckRisk,
) -> bool {
    ack_risks.contains(&expected) || approved_risks.contains(&expected)
}

fn merge_ack_risks(ack_risks: &[AckRisk], approved_risks: &BTreeSet<AckRisk>) -> Vec<AckRisk> {
    let mut merged = approved_risks.iter().copied().collect::<Vec<_>>();
    for ack_risk in ack_risks {
        if !merged.contains(ack_risk) {
            merged.push(*ack_risk);
        }
    }
    merged
}

fn policy_profile_label(profile: PolicyProfile) -> &'static str {
    match profile {
        PolicyProfile::ResearchReadOnly => "research-read-only",
        PolicyProfile::ResearchRestricted => "research-restricted",
        PolicyProfile::InteractiveReview => "interactive-review",
        PolicyProfile::InteractiveSupervisedAuth => "interactive-supervised-auth",
        PolicyProfile::InteractiveSupervisedWrite => "interactive-supervised-write",
    }
}

fn recommended_policy_profile(policy: &PolicyReport) -> PolicyProfile {
    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::HighRiskWrite)
    {
        return PolicyProfile::InteractiveSupervisedWrite;
    }

    if policy.signals.iter().any(|signal| {
        matches!(
            signal.kind,
            touch_browser_contracts::PolicySignalKind::BotChallenge
                | touch_browser_contracts::PolicySignalKind::MfaChallenge
                | touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow
        )
    }) {
        return PolicyProfile::InteractiveSupervisedAuth;
    }

    PolicyProfile::InteractiveReview
}

fn promoted_policy_profile_for_risks(
    current: PolicyProfile,
    approved_risks: &BTreeSet<AckRisk>,
) -> PolicyProfile {
    if approved_risks.contains(&AckRisk::HighRiskWrite) {
        return PolicyProfile::InteractiveSupervisedWrite;
    }

    if approved_risks.contains(&AckRisk::Challenge)
        || approved_risks.contains(&AckRisk::Mfa)
        || approved_risks.contains(&AckRisk::Auth)
    {
        return match current {
            PolicyProfile::InteractiveSupervisedWrite => PolicyProfile::InteractiveSupervisedWrite,
            _ => PolicyProfile::InteractiveSupervisedAuth,
        };
    }

    current
}

fn required_ack_risks(policy: &PolicyReport) -> Vec<String> {
    let mut risks = BTreeSet::new();
    for signal in &policy.signals {
        match signal.kind {
            touch_browser_contracts::PolicySignalKind::BotChallenge => {
                risks.insert(AckRisk::Challenge);
            }
            touch_browser_contracts::PolicySignalKind::MfaChallenge => {
                risks.insert(AckRisk::Mfa);
            }
            touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow => {
                risks.insert(AckRisk::Auth);
            }
            touch_browser_contracts::PolicySignalKind::HighRiskWrite => {
                risks.insert(AckRisk::HighRiskWrite);
            }
            _ => {}
        }
    }

    risks
        .into_iter()
        .map(|risk| ack_risk_label(risk).to_string())
        .collect()
}

fn checkpoint_provider_hints(snapshot: &SnapshotDocument, policy: &PolicyReport) -> Vec<String> {
    let source_url = snapshot.source.source_url.to_ascii_lowercase();
    let mut hints = BTreeSet::new();

    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow)
    {
        if source_url.contains("github.com") {
            hints.insert("github-auth".to_string());
        } else if source_url.contains("accounts.google.com") || source_url.contains("google.com") {
            hints.insert("google-auth".to_string());
        } else if source_url.contains("login.microsoftonline.com")
            || source_url.contains("microsoft.com")
        {
            hints.insert("microsoft-auth".to_string());
        } else if source_url.contains("okta.") {
            hints.insert("okta-auth".to_string());
        } else if source_url.contains("auth0.") {
            hints.insert("auth0-auth".to_string());
        } else {
            hints.insert("generic-auth".to_string());
        }
    }

    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::BotChallenge)
    {
        if source_url.contains("google.com")
            && snapshot
                .source
                .title
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase()
                .contains("recaptcha")
        {
            hints.insert("google-recaptcha".to_string());
        } else {
            hints.insert("generic-bot-challenge".to_string());
        }
    }

    if policy
        .signals
        .iter()
        .any(|signal| signal.kind == touch_browser_contracts::PolicySignalKind::HighRiskWrite)
    {
        if source_url.contains("stripe") {
            hints.insert("stripe-checkout-like".to_string());
        } else if source_url.contains("paypal") {
            hints.insert("paypal-checkout-like".to_string());
        } else if source_url.contains("shopify") {
            hints.insert("shopify-checkout-like".to_string());
        } else {
            hints.insert("generic-high-risk-write".to_string());
        }
    }

    hints.into_iter().collect()
}

fn checkpoint_approval_panel(
    provider_hints: &[String],
    required_ack_risks: &[String],
    approved_risks: &[String],
    active_profile: PolicyProfile,
    recommended_profile: PolicyProfile,
    policy: &PolicyReport,
) -> Value {
    let severity = match policy.decision {
        touch_browser_contracts::PolicyDecision::Block => "block",
        touch_browser_contracts::PolicyDecision::Review => "review",
        touch_browser_contracts::PolicyDecision::Allow => "allow",
    };
    let mut actions = vec![json!({
        "id": "refresh",
        "label": "Refresh after manual continuation",
        "command": "refresh",
    })];

    if !required_ack_risks.is_empty() {
        actions.insert(
            0,
            json!({
                "id": "approve",
                "label": "Approve supervised continuation",
                "command": "approve",
                "requiredAckRisks": required_ack_risks,
            }),
        );
    }

    if required_ack_risks
        .iter()
        .any(|risk| risk == "auth" || risk == "mfa")
    {
        actions.push(json!({
            "id": "store-secret",
            "label": "Store daemon secret for supervised auth",
            "command": "secret.store",
        }));
    }

    json!({
        "title": "Supervised continuation required",
        "severity": severity,
        "provider": provider_hints.first().cloned().unwrap_or_else(|| "generic".to_string()),
        "activePolicyProfile": policy_profile_label(active_profile),
        "recommendedPolicyProfile": policy_profile_label(recommended_profile),
        "requiredAckRisks": required_ack_risks,
        "approvedRisks": approved_risks,
        "actions": actions,
    })
}

fn checkpoint_playbook(
    provider_hints: &[String],
    required_ack_risks: &[String],
    approved_risks: &[String],
    snapshot: &SnapshotDocument,
    recommended_profile: PolicyProfile,
) -> Value {
    let provider = provider_hints
        .first()
        .cloned()
        .unwrap_or_else(|| "generic".to_string());
    let mut steps = match provider.as_str() {
        "github-auth" => vec![
            "Open the session in headed mode and continue on the GitHub login surface.".to_string(),
            "Type username/password or store the password with the daemon secret store.".to_string(),
            "Approve auth supervision before submit, then refresh after the page advances.".to_string(),
        ],
        "google-auth" => vec![
            "Keep the browser headed and complete the Google account challenge on the live page.".to_string(),
            "If prompted for OTP or device approval, complete the checkpoint manually and refresh.".to_string(),
            "Approve auth or MFA supervision before retrying submit.".to_string(),
        ],
        "microsoft-auth" | "okta-auth" | "auth0-auth" | "generic-auth" => vec![
            "Continue only in headed mode on the live authentication provider.".to_string(),
            "Store sensitive secrets in daemon memory rather than plain CLI arguments.".to_string(),
            "Approve auth or MFA supervision before submit, then refresh once the provider advances.".to_string(),
        ],
        "google-recaptcha" | "generic-bot-challenge" => vec![
            "Switch to headed mode and complete the visible human verification checkpoint.".to_string(),
            "Do not retry automated clicks until the challenge is cleared.".to_string(),
            "Approve challenge supervision and refresh the captured state after the page advances.".to_string(),
        ],
        "stripe-checkout-like" | "paypal-checkout-like" | "shopify-checkout-like" | "generic-high-risk-write" => vec![
            "Treat the next step as a supervised write boundary.".to_string(),
            "Review the target form/button and confirm the exact side effect before approval.".to_string(),
            "Approve high-risk write only for the intended action, then refresh immediately after completion.".to_string(),
        ],
        _ => vec![
            "Continue in headed mode when the page requires manual supervision.".to_string(),
            "Use approve to persist supervised risk acknowledgements for this session.".to_string(),
            "Refresh the browser-backed snapshot after the live page changes.".to_string(),
        ],
    };

    if required_ack_risks.iter().any(|risk| risk == "mfa") {
        steps.push("If an OTP or verification code is required, store it through the daemon secret store and use typeSecret.".to_string());
    }

    let sensitive_targets = snapshot
        .blocks
        .iter()
        .filter(|block| currentish_block_is_sensitive(block))
        .take(6)
        .map(|block| {
            json!({
                "ref": block.stable_ref,
                "text": block.text,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "provider": provider,
        "recommendedPolicyProfile": policy_profile_label(recommended_profile),
        "requiredAckRisks": required_ack_risks,
        "approvedRisks": approved_risks,
        "steps": steps,
        "sensitiveTargets": sensitive_targets,
    })
}

fn checkpoint_candidates(snapshot: &SnapshotDocument) -> Vec<Value> {
    snapshot
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.kind,
                touch_browser_contracts::SnapshotBlockKind::Form
                    | touch_browser_contracts::SnapshotBlockKind::Input
                    | touch_browser_contracts::SnapshotBlockKind::Button
                    | touch_browser_contracts::SnapshotBlockKind::Link
            )
        })
        .take(12)
        .map(|block| {
            json!({
                "kind": block.kind,
                "ref": block.stable_ref,
                "text": block.text,
            })
        })
        .collect()
}

fn currentish_block_is_sensitive(block: &SnapshotBlock) -> bool {
    let text = block.text.to_ascii_lowercase();
    let name = block
        .attributes
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    let input_type = block
        .attributes
        .get("inputType")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();

    input_type == "password"
        || name.contains("pass")
        || name == "otp"
        || name.ends_with("_otp")
        || name.contains("token")
        || name.contains("verification")
        || text.contains("password")
        || text == "otp"
        || text.contains("verification")
}

fn policy_requires_ack(
    session: &ReadOnlySession,
    policy: &PolicyReport,
    action: ActionName,
    target_ref: Option<&str>,
) -> Option<(AckRisk, &'static str)> {
    for signal in &policy.signals {
        let matches_target = target_ref.is_none()
            || signal.stable_ref.as_deref() == target_ref
            || signal.stable_ref.is_none();

        if !matches_target {
            continue;
        }

        match signal.kind {
            touch_browser_contracts::PolicySignalKind::BotChallenge => {
                return Some((
                    AckRisk::Challenge,
                    "Detected a likely bot or CAPTCHA checkpoint. Re-open in headed mode, complete the human checkpoint manually, then retry with `--ack-risk challenge` or run `refresh` after the page advances.",
                ));
            }
            touch_browser_contracts::PolicySignalKind::MfaChallenge => {
                return Some((
                    AckRisk::Mfa,
                    "Detected a likely MFA or verification checkpoint. Use headed mode, complete the challenge manually or provide a daemon secret, then retry with `--ack-risk mfa`.",
                ));
            }
            touch_browser_contracts::PolicySignalKind::SensitiveAuthFlow => {
                let requires_ack = action != ActionName::Type
                    || target_ref.is_some_and(|target_ref| {
                        current_snapshot_ref_is_sensitive(session, target_ref)
                    });
                if requires_ack {
                    return Some((
                        AckRisk::Auth,
                        "Detected a credential-bearing authentication flow. Continue only in headed mode with explicit `--ack-risk auth`.",
                    ));
                }
            }
            touch_browser_contracts::PolicySignalKind::HighRiskWrite => {
                if matches!(action, ActionName::Click | ActionName::Submit) {
                    return Some((
                        AckRisk::HighRiskWrite,
                        "Detected a high-risk write action. Continue only in headed mode with explicit `--ack-risk high-risk-write`.",
                    ));
                }
            }
            _ => {}
        }
    }

    None
}

fn preflight_ref_action(
    persisted: &BrowserCliSession,
    kernel: &PolicyKernel,
    action: ActionName,
    target_ref: &str,
    message: &str,
    session_file: &Path,
) -> Option<SessionCommandOutput> {
    let policy =
        current_policy_with_allowlist(&persisted.session, kernel, &persisted.allowlisted_domains)?;
    if !policy
        .blocked_refs
        .iter()
        .any(|blocked| blocked == target_ref)
    {
        return None;
    }

    let action_result = reject_action(
        action,
        ActionFailureKind::PolicyBlocked,
        message,
        Some(policy),
    );

    Some(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state.clone(),
        session_file: session_file.display().to_string(),
    })
}

fn preflight_session_block(
    persisted: &BrowserCliSession,
    kernel: &PolicyKernel,
    action: ActionName,
    message: &str,
    session_file: &Path,
) -> Option<SessionCommandOutput> {
    let policy =
        current_policy_with_allowlist(&persisted.session, kernel, &persisted.allowlisted_domains)?;
    if policy.decision != touch_browser_contracts::PolicyDecision::Block {
        return None;
    }

    if action == ActionName::Paginate
        && policy.source_risk == SourceRisk::Low
        && !policy.signals.is_empty()
        && policy.signals.iter().all(|signal| {
            signal.kind == touch_browser_contracts::PolicySignalKind::DomainNotAllowlisted
        })
    {
        return None;
    }

    let action_result = reject_action(
        action,
        ActionFailureKind::PolicyBlocked,
        message,
        Some(policy),
    );

    Some(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state.clone(),
        session_file: session_file.display().to_string(),
    })
}

struct InteractivePreflightOptions<'a> {
    action: ActionName,
    target_ref: Option<&'a str>,
    headed: bool,
    ack_risks: &'a [AckRisk],
    message: &'a str,
    session_file: &'a Path,
}

fn preflight_interactive_action(
    persisted: &BrowserCliSession,
    kernel: &PolicyKernel,
    options: InteractivePreflightOptions<'_>,
) -> Option<SessionCommandOutput> {
    let InteractivePreflightOptions {
        action,
        target_ref,
        headed,
        ack_risks,
        message,
        session_file,
    } = options;
    let policy =
        current_policy_with_allowlist(&persisted.session, kernel, &persisted.allowlisted_domains)?;

    if persisted.allowlisted_domains.is_empty() {
        let action_result = reject_action(
            action,
            ActionFailureKind::PolicyBlocked,
            "Interactive browser actions require at least one `--allow-domain` boundary.",
            Some(policy),
        );
        return Some(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state.clone(),
            session_file: session_file.display().to_string(),
        });
    }

    if policy.signals.iter().any(|signal| {
        signal.kind == touch_browser_contracts::PolicySignalKind::DomainNotAllowlisted
            && signal.stable_ref.is_none()
    }) {
        let action_result = reject_action(
            action,
            ActionFailureKind::PolicyBlocked,
            "Interactive browser actions require the current page host to be inside the allowlist.",
            Some(policy),
        );
        return Some(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state.clone(),
            session_file: session_file.display().to_string(),
        });
    }

    if policy.source_risk == SourceRisk::Hostile {
        let action_result = reject_action(
            action,
            ActionFailureKind::PolicyBlocked,
            "Interactive browser actions are blocked on hostile sources.",
            Some(policy),
        );
        return Some(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state.clone(),
            session_file: session_file.display().to_string(),
        });
    }

    if let Some((required_ack, detail)) =
        policy_requires_ack(&persisted.session, &policy, action.clone(), target_ref)
    {
        let requires_headed = persisted
            .session
            .current_snapshot_record()
            .map(|record| record.snapshot.source.source_url.as_str())
            .is_none_or(|source_url| !is_fixture_target(source_url));
        if requires_headed && !headed {
            let action_result = reject_action(
                action,
                ActionFailureKind::PolicyBlocked,
                &format!("{detail} Headed mode is required for supervised continuation."),
                Some(policy),
            );
            return Some(SessionCommandOutput {
                action: action_result.clone(),
                result: action_result,
                session_state: persisted.session.state.clone(),
                session_file: session_file.display().to_string(),
            });
        }

        if !has_ack_risk(ack_risks, &persisted.approved_risks, required_ack) {
            let action_result = reject_action(
                action,
                ActionFailureKind::PolicyBlocked,
                &format!(
                    "{detail} Re-run with `--ack-risk {}` once you want to cross this boundary.",
                    ack_risk_label(required_ack)
                ),
                Some(policy),
            );
            return Some(SessionCommandOutput {
                action: action_result.clone(),
                result: action_result,
                session_state: persisted.session.state.clone(),
                session_file: session_file.display().to_string(),
            });
        }
    }

    if let Some(target_ref) = target_ref {
        if policy
            .blocked_refs
            .iter()
            .any(|blocked| blocked == target_ref)
        {
            let action_result = reject_action(
                action,
                ActionFailureKind::PolicyBlocked,
                message,
                Some(policy),
            );
            return Some(SessionCommandOutput {
                action: action_result.clone(),
                result: action_result,
                session_state: persisted.session.state.clone(),
                session_file: session_file.display().to_string(),
            });
        }
    }

    None
}

fn slot_timestamp(slot: usize, seconds: usize) -> String {
    let hour = slot / 60;
    let minute = slot % 60;
    format!("2026-03-14T{hour:02}:{minute:02}:{seconds:02}+09:00")
}

fn is_fixture_target(target: &str) -> bool {
    target.starts_with("fixture://")
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should exist")
}

fn usage() -> String {
    [
        "Usage:",
        "  Stable research commands:",
        "  touch-browser search <query> [--engine google|brave] [--headed] [--profile-dir <path>] [--budget <tokens>] [--session-file <path>]",
        "  touch-browser search-open-result --rank <number> [--engine google|brave] [--session-file <path>] [--headed]",
        "  touch-browser search-open-top [--limit <count>] [--engine google|brave] [--session-file <path>] [--headed]",
        "  touch-browser open <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser snapshot <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser compact-view <target> [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser read-view <target> [--browser] [--headed] [--main-only] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser extract <target> --claim <statement> [--claim <statement> ...] [--verifier-command <shell-command>] [--browser] [--headed] [--budget <tokens>] [--session-file <path>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser policy <target> [--browser] [--headed] [--budget <tokens>] [--source-risk low|medium|hostile] [--source-label <label>] [--allow-domain <host> ...]",
        "  touch-browser session-snapshot --session-file <path>",
        "  touch-browser session-compact --session-file <path>",
        "  touch-browser session-extract --session-file <path> --claim <statement> [--claim <statement> ...] [--verifier-command <shell-command>]",
        "  touch-browser session-read --session-file <path> [--main-only]",
        "  touch-browser session-synthesize --session-file <path> [--note-limit <count>] [--format json|markdown]",
        "  touch-browser follow --session-file <path> --ref <stable-ref> [--headed]",
        "  touch-browser paginate --session-file <path> --direction next|prev [--headed]",
        "  touch-browser expand --session-file <path> --ref <stable-ref> [--headed]",
        "  touch-browser browser-replay --session-file <path>",
        "  touch-browser session-close --session-file <path>",
        "  touch-browser telemetry-summary",
        "  touch-browser telemetry-recent [--limit <count>]",
        "  touch-browser replay <scenario-name>",
        "  touch-browser memory-summary [--steps <even-number>]",
        "  touch-browser serve",
        "  Experimental supervised commands:",
        "  touch-browser refresh --session-file <path> [--headed]",
        "  touch-browser checkpoint --session-file <path>",
        "  touch-browser session-policy --session-file <path>",
        "  touch-browser session-profile --session-file <path>",
        "  touch-browser set-profile --session-file <path> --profile research-read-only|research-restricted|interactive-review|interactive-supervised-auth|interactive-supervised-write",
        "  touch-browser approve --session-file <path> --risk challenge|mfa|auth|high-risk-write [--risk ...]",
        "  touch-browser click --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]",
        "  touch-browser type --session-file <path> --ref <stable-ref> --value <text> [--headed] [--sensitive] [--ack-risk challenge|mfa|auth ...]",
        "  touch-browser submit --session-file <path> --ref <stable-ref> [--headed] [--ack-risk challenge|mfa|auth|high-risk-write ...]",
    ]
    .join("\n")
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CliCommand {
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
struct TargetOptions {
    target: String,
    budget: usize,
    source_risk: Option<SourceRisk>,
    source_label: Option<String>,
    allowlisted_domains: Vec<String>,
    browser: bool,
    headed: bool,
    main_only: bool,
    session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchOptions {
    query: String,
    engine: SearchEngine,
    budget: usize,
    headed: bool,
    profile_dir: Option<PathBuf>,
    session_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchOpenResultOptions {
    engine: SearchEngine,
    session_file: Option<PathBuf>,
    rank: usize,
    headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchOpenTopOptions {
    engine: SearchEngine,
    session_file: Option<PathBuf>,
    limit: usize,
    headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExtractOptions {
    target: String,
    budget: usize,
    source_risk: Option<SourceRisk>,
    source_label: Option<String>,
    allowlisted_domains: Vec<String>,
    browser: bool,
    headed: bool,
    session_file: Option<PathBuf>,
    claims: Vec<String>,
    verifier_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionFileOptions {
    session_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionReadOptions {
    session_file: PathBuf,
    main_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionExtractOptions {
    session_file: PathBuf,
    claims: Vec<String>,
    verifier_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionSynthesizeOptions {
    session_file: PathBuf,
    note_limit: usize,
    format: OutputFormat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum OutputFormat {
    Json,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionProfileSetOptions {
    session_file: PathBuf,
    profile: PolicyProfile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TelemetryRecentOptions {
    limit: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionRefreshOptions {
    session_file: PathBuf,
    headed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum AckRisk {
    Challenge,
    Mfa,
    Auth,
    HighRiskWrite,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ApproveOptions {
    session_file: PathBuf,
    ack_risks: Vec<AckRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FollowOptions {
    session_file: PathBuf,
    target_ref: String,
    headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ClickOptions {
    session_file: PathBuf,
    target_ref: String,
    headed: bool,
    ack_risks: Vec<AckRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TypeOptions {
    session_file: PathBuf,
    target_ref: String,
    value: String,
    headed: bool,
    sensitive: bool,
    ack_risks: Vec<AckRisk>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SubmitOptions {
    session_file: PathBuf,
    target_ref: String,
    headed: bool,
    ack_risks: Vec<AckRisk>,
    extra_prefill: Vec<SecretPrefill>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PaginationDirection {
    Next,
    Prev,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PaginateOptions {
    session_file: PathBuf,
    direction: PaginationDirection,
    headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpandOptions {
    session_file: PathBuf,
    target_ref: String,
    headed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SecretPrefill {
    target_ref: String,
    value: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMetadata {
    title: String,
    source_uri: String,
    html_path: String,
    risk: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtractCommandOutput {
    open: ActionResult,
    extract: ActionResult,
    session_state: SessionState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyCommandOutput {
    policy: PolicyReport,
    session_state: SessionState,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayCommandOutput {
    session_state: SessionState,
    replay_transcript: ReplayTranscript,
    snapshot_count: usize,
    evidence_report_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemorySummaryOutput {
    requested_actions: usize,
    action_count: usize,
    session_state: SessionState,
    memory_summary: MemorySessionSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionCommandOutput {
    action: ActionResult,
    result: ActionResult,
    session_state: SessionState,
    session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionExtractCommandOutput {
    extract: ActionResult,
    result: ActionResult,
    session_state: SessionState,
    session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionPolicyCommandOutput {
    policy: PolicyReport,
    result: PolicyReport,
    session_state: SessionState,
    session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionSynthesisCommandOutput {
    report: SessionSynthesisReport,
    result: SessionSynthesisReport,
    format: OutputFormat,
    #[serde(skip_serializing_if = "Option::is_none")]
    markdown: Option<String>,
    session_state: SessionState,
    session_file: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionCloseCommandOutput {
    session_file: String,
    removed: bool,
    result: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompactSnapshotOutput {
    source_url: String,
    compact_text: String,
    reading_compact_text: String,
    navigation_compact_text: String,
    line_count: usize,
    char_count: usize,
    approx_tokens: usize,
    ref_index: Vec<CompactRefIndexEntry>,
    navigation_ref_index: Vec<CompactRefIndexEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_state: Option<SessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_file: Option<String>,
}

impl CompactSnapshotOutput {
    fn new(
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
struct ReadViewOutput {
    source_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    source_title: Option<String>,
    markdown_text: String,
    main_only: bool,
    line_count: usize,
    char_count: usize,
    approx_tokens: usize,
    ref_index: Vec<CompactRefIndexEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_state: Option<SessionState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_file: Option<String>,
}

impl ReadViewOutput {
    fn new(
        snapshot: &SnapshotDocument,
        session_state: Option<SessionState>,
        session_file: Option<String>,
        main_only: bool,
    ) -> Self {
        let markdown_text = if main_only {
            render_main_read_view_markdown(snapshot)
        } else {
            let preferred_markdown = render_main_read_view_markdown(snapshot);
            if preferred_markdown.is_empty() {
                render_read_view_markdown(snapshot)
            } else {
                preferred_markdown
            }
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
struct BrowserReplayCommandOutput {
    replayed_actions: usize,
    compact_text: String,
    session_state: SessionState,
}

#[derive(Debug, Error)]
enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("observation error: {0}")]
    Observation(#[from] touch_browser_observation::ObservationError),
    #[error("runtime error: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("acquisition error: {0}")]
    Acquisition(#[from] AcquisitionError),
    #[error("telemetry error: {0}")]
    Telemetry(#[from] TelemetryError),
    #[error("adapter error: {0}")]
    Adapter(String),
    #[error("verifier error: {0}")]
    Verifier(String),
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::json;
    use touch_browser_contracts::{
        SearchReport, SearchResultItem, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole,
        SnapshotBudget, SnapshotDocument, SnapshotEvidence, SnapshotSource, SourceType,
    };

    use super::{
        browser_context_dir_for_session_file, build_search_report,
        derived_search_result_session_file, dispatch, load_browser_cli_session, parse_command,
        save_browser_cli_session, AckRisk, ApproveOptions, CliCommand, ClickOptions, ExpandOptions,
        ExtractOptions, FollowOptions, OutputFormat, PaginateOptions, PaginationDirection,
        PolicyProfile, SearchActionActor, SearchEngine, SearchOpenResultOptions,
        SearchOpenTopOptions, SearchOptions, SearchReportStatus, SessionExtractOptions,
        SessionFileOptions, SessionProfileSetOptions, SessionReadOptions, SessionRefreshOptions,
        SessionSynthesizeOptions, SubmitOptions, TargetOptions, TelemetryRecentOptions,
        TypeOptions, DEFAULT_OPENED_AT, DEFAULT_REQUESTED_TOKENS, DEFAULT_SEARCH_TOKENS,
    };

    fn temp_session_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        std::env::temp_dir().join(format!("touch-browser-{name}-{nanos}.json"))
    }

    #[test]
    fn parses_extract_command_with_multiple_claims() {
        let command = parse_command(&[
            "extract".to_string(),
            "fixture://research/citation-heavy/pricing".to_string(),
            "--claim".to_string(),
            "The Starter plan costs $29 per month.".to_string(),
            "--claim".to_string(),
            "There is an Enterprise plan.".to_string(),
        ])
        .expect("extract command should parse");

        assert_eq!(
            command,
            CliCommand::Extract(ExtractOptions {
                target: "fixture://research/citation-heavy/pricing".to_string(),
                budget: DEFAULT_REQUESTED_TOKENS,
                source_risk: None,
                source_label: None,
                allowlisted_domains: Vec::new(),
                browser: false,
                headed: false,
                session_file: None,
                claims: vec![
                    "The Starter plan costs $29 per month.".to_string(),
                    "There is an Enterprise plan.".to_string(),
                ],
                verifier_command: None,
            })
        );
    }

    #[test]
    fn parses_search_command_with_engine_and_session_file() {
        let command = parse_command(&[
            "search".to_string(),
            "lambda timeout".to_string(),
            "--engine".to_string(),
            "brave".to_string(),
            "--session-file".to_string(),
            "/tmp/search-session.json".to_string(),
            "--headed".to_string(),
        ])
        .expect("search command should parse");

        assert_eq!(
            command,
            CliCommand::Search(SearchOptions {
                query: "lambda timeout".to_string(),
                engine: SearchEngine::Brave,
                budget: DEFAULT_SEARCH_TOKENS,
                headed: true,
                profile_dir: None,
                session_file: Some(PathBuf::from("/tmp/search-session.json")),
            })
        );
    }

    #[test]
    fn parses_search_command_with_profile_dir() {
        let command = parse_command(&[
            "search".to_string(),
            "lambda timeout".to_string(),
            "--profile-dir".to_string(),
            "/tmp/dedicated-search-profile".to_string(),
        ])
        .expect("search command with profile dir should parse");

        assert_eq!(
            command,
            CliCommand::Search(SearchOptions {
                query: "lambda timeout".to_string(),
                engine: SearchEngine::Google,
                budget: DEFAULT_SEARCH_TOKENS,
                headed: false,
                profile_dir: Some(PathBuf::from("/tmp/dedicated-search-profile")),
                session_file: None,
            })
        );
    }

    #[test]
    fn parses_search_open_result_command() {
        let command = parse_command(&[
            "search-open-result".to_string(),
            "--session-file".to_string(),
            "/tmp/search-session.json".to_string(),
            "--rank".to_string(),
            "2".to_string(),
        ])
        .expect("search-open-result command should parse");

        assert_eq!(
            command,
            CliCommand::SearchOpenResult(SearchOpenResultOptions {
                engine: SearchEngine::Google,
                session_file: Some(PathBuf::from("/tmp/search-session.json")),
                rank: 2,
                headed: false,
            })
        );
    }

    #[test]
    fn parses_search_open_top_command() {
        let command = parse_command(&[
            "search-open-top".to_string(),
            "--engine".to_string(),
            "brave".to_string(),
            "--limit".to_string(),
            "2".to_string(),
            "--headless".to_string(),
        ])
        .expect("search-open-top command should parse");

        assert_eq!(
            command,
            CliCommand::SearchOpenTop(SearchOpenTopOptions {
                engine: SearchEngine::Brave,
                session_file: None,
                limit: 2,
                headed: false,
            })
        );
    }

    #[test]
    fn parses_extract_command_with_verifier_hook() {
        let command = parse_command(&[
            "extract".to_string(),
            "fixture://research/citation-heavy/pricing".to_string(),
            "--claim".to_string(),
            "The Starter plan costs $29 per month.".to_string(),
            "--verifier-command".to_string(),
            "printf '{\"outcomes\":[]}'".to_string(),
        ])
        .expect("extract command with verifier should parse");

        assert_eq!(
            command,
            CliCommand::Extract(ExtractOptions {
                target: "fixture://research/citation-heavy/pricing".to_string(),
                budget: DEFAULT_REQUESTED_TOKENS,
                source_risk: None,
                source_label: None,
                allowlisted_domains: Vec::new(),
                browser: false,
                headed: false,
                session_file: None,
                claims: vec!["The Starter plan costs $29 per month.".to_string()],
                verifier_command: Some("printf '{\"outcomes\":[]}'".to_string()),
            })
        );
    }

    #[test]
    fn parses_session_synthesize_markdown_format() {
        let command = parse_command(&[
            "session-synthesize".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--format".to_string(),
            "markdown".to_string(),
        ])
        .expect("session-synthesize command should parse");

        assert_eq!(
            command,
            CliCommand::SessionSynthesize(SessionSynthesizeOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                note_limit: 12,
                format: OutputFormat::Markdown,
            })
        );
    }

    #[test]
    fn dispatches_read_view_for_fixture_target() {
        let output = dispatch(CliCommand::ReadView(TargetOptions {
            target: "fixture://research/static-docs/getting-started".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            main_only: false,
            session_file: None,
        }))
        .expect("read-view should succeed");

        let markdown = output["markdownText"]
            .as_str()
            .expect("markdown text should be present");
        assert!(markdown.starts_with('#'));
        assert!(markdown.contains("Getting Started"));
        assert!(output["approxTokens"].as_u64().unwrap_or(0) > 0);
    }

    #[test]
    fn structures_google_style_search_results_from_snapshot_blocks() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                source_type: SourceType::Playwright,
                title: Some("lambda timeout - Google Search".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: DEFAULT_SEARCH_TOKENS,
                estimated_tokens: 256,
                emitted_tokens: 256,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:aws-lambda-quotas".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Lambda quotas".to_string(),
                    attributes: std::collections::BTreeMap::from([(
                        "href".to_string(),
                        json!("https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html"),
                    )]),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > a:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:aws-lambda-quotas-snippet".to_string(),
                    role: SnapshotBlockRole::Supporting,
                    text: "Function timeout: 900 seconds (15 minutes).".to_string(),
                    attributes: Default::default(),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:google-help".to_string(),
                    role: SnapshotBlockRole::PrimaryNav,
                    text: "Google Help".to_string(),
                    attributes: std::collections::BTreeMap::from([(
                        "href".to_string(),
                        json!("https://support.google.com/websearch"),
                    )]),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > nav > a:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = build_search_report(
            SearchEngine::Google,
            "lambda timeout",
            "https://www.google.com/search?q=lambda+timeout",
            &snapshot,
            "<html></html>",
            "https://www.google.com/search?q=lambda+timeout",
            "2026-04-05T00:00:00+09:00",
        );

        assert_eq!(report.status, SearchReportStatus::Ready);
        assert_eq!(report.result_count, 1);
        assert_eq!(report.results[0].rank, 1);
        assert_eq!(report.results[0].domain, "docs.aws.amazon.com".to_string());
        assert_eq!(
            report.results[0].recommended_surface.as_deref(),
            Some("extract")
        );
        assert!(report.next_action_hints.iter().any(|hint| {
            hint.action == "open-top" && hint.actor == SearchActionActor::Ai && hint.can_auto_run
        }));
    }

    #[test]
    fn structures_search_results_from_html_when_snapshot_is_sparse() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://search.brave.com/search?q=lambda+timeout".to_string(),
                source_type: SourceType::Playwright,
                title: Some("lambda timeout - Brave Search".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: DEFAULT_SEARCH_TOKENS,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: Vec::new(),
        };

        let html = r#"
            <html>
              <body>
                <main>
                  <div class="snippet" data-type="web">
                    <a href="https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html">
                      Lambda quotas
                    </a>
                    <p>Function timeout: 900 seconds (15 minutes).</p>
                  </div>
                </main>
              </body>
            </html>
        "#;

        let report = build_search_report(
            SearchEngine::Brave,
            "lambda timeout",
            "https://search.brave.com/search?q=lambda+timeout",
            &snapshot,
            html,
            "https://search.brave.com/search?q=lambda+timeout",
            "2026-04-05T00:00:00+09:00",
        );

        assert_eq!(report.status, SearchReportStatus::Ready);
        assert_eq!(report.result_count, 1);
        assert_eq!(report.results[0].title, "Lambda quotas");
        assert_eq!(
            report.results[0].snippet.as_deref(),
            Some("Function timeout: 900 seconds (15 minutes).")
        );
    }

    #[test]
    fn marks_google_sorry_pages_as_search_challenges() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: SnapshotSource {
                source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                source_type: SourceType::Playwright,
                title: Some("Traffic verification".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: DEFAULT_SEARCH_TOKENS,
                estimated_tokens: 96,
                emitted_tokens: 96,
                truncated: false,
            },
            blocks: vec![SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: SnapshotBlockKind::Text,
                stable_ref: "rmain:text:captcha".to_string(),
                role: SnapshotBlockRole::Supporting,
                text: "Google detected unusual traffic and requires reCAPTCHA verification."
                    .to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://www.google.com/sorry/index".to_string(),
                    source_type: SourceType::Playwright,
                    dom_path_hint: Some("html > body > main".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = build_search_report(
            SearchEngine::Google,
            "lambda timeout",
            "https://www.google.com/search?q=lambda+timeout",
            &snapshot,
            "<html></html>",
            "https://www.google.com/sorry/index?q=test",
            "2026-04-05T00:00:00+09:00",
        );

        assert_eq!(report.status, SearchReportStatus::Challenge);
        assert_eq!(report.result_count, 0);
        assert!(report.next_action_hints.iter().any(|hint| {
            hint.action == "complete-challenge"
                && hint.actor == SearchActionActor::Human
                && hint.headed_required
                && !hint.can_auto_run
        }));
    }

    #[test]
    fn search_open_result_preserves_latest_search_state() {
        let session_file = temp_session_path("search-open-result-preserve");
        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-pagination".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let mut persisted =
            load_browser_cli_session(&session_file).expect("session should load after open");
        persisted.latest_search = Some(SearchReport {
            version: "1.0.0".to_string(),
            generated_at: DEFAULT_OPENED_AT.to_string(),
            engine: SearchEngine::Google,
            query: "browser pagination".to_string(),
            search_url: "https://www.google.com/search?q=browser+pagination".to_string(),
            final_url: "https://www.google.com/search?q=browser+pagination".to_string(),
            status: SearchReportStatus::Ready,
            status_detail: None,
            result_count: 1,
            results: vec![SearchResultItem {
                rank: 1,
                title: "Browser Pagination".to_string(),
                url: "fixture://research/navigation/browser-pagination".to_string(),
                domain: "fixture.local".to_string(),
                snippet: Some("Fixture search result".to_string()),
                stable_ref: None,
                official_likely: true,
                selection_score: Some(1.0),
                recommended_surface: Some("read-view".to_string()),
            }],
            recommended_result_ranks: vec![1],
            next_action_hints: Vec::new(),
        });
        save_browser_cli_session(&session_file, &persisted)
            .expect("session should save with search state");

        dispatch(CliCommand::SearchOpenResult(SearchOpenResultOptions {
            engine: SearchEngine::Google,
            session_file: Some(session_file.clone()),
            rank: 1,
            headed: false,
        }))
        .expect("search-open-result should succeed");

        let refreshed =
            load_browser_cli_session(&session_file).expect("session should reload after open");
        let latest_search = refreshed
            .latest_search
            .expect("latest search should still be present after opening a result");
        assert_eq!(latest_search.result_count, 1);
        assert_eq!(latest_search.results[0].rank, 1);

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session close should clean search session");
    }

    #[test]
    fn search_open_top_inherits_external_profile_directory() {
        let session_file = temp_session_path("search-open-top-profile");
        let profile_dir = std::env::temp_dir().join(format!(
            "touch-browser-external-profile-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be monotonic")
                .as_nanos()
        ));
        fs::create_dir_all(&profile_dir).expect("external profile dir should exist");

        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-pagination".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let mut persisted =
            load_browser_cli_session(&session_file).expect("session should load after open");
        if let Some(context_dir) = persisted.browser_context_dir.as_ref() {
            let context_path = PathBuf::from(context_dir);
            if context_path.exists() {
                fs::remove_dir_all(context_path).expect("managed context dir should clean up");
            }
        }
        persisted.browser_context_dir = None;
        persisted.browser_profile_dir = Some(profile_dir.display().to_string());
        persisted.latest_search = Some(SearchReport {
            version: "1.0.0".to_string(),
            generated_at: DEFAULT_OPENED_AT.to_string(),
            engine: SearchEngine::Google,
            query: "browser pagination".to_string(),
            search_url: "https://www.google.com/search?q=browser+pagination".to_string(),
            final_url: "https://www.google.com/search?q=browser+pagination".to_string(),
            status: SearchReportStatus::Ready,
            status_detail: None,
            result_count: 1,
            results: vec![SearchResultItem {
                rank: 1,
                title: "Browser Pagination".to_string(),
                url: "fixture://research/navigation/browser-pagination".to_string(),
                domain: "fixture.local".to_string(),
                snippet: Some("Fixture search result".to_string()),
                stable_ref: None,
                official_likely: true,
                selection_score: Some(1.0),
                recommended_surface: Some("read-view".to_string()),
            }],
            recommended_result_ranks: vec![1],
            next_action_hints: Vec::new(),
        });
        save_browser_cli_session(&session_file, &persisted)
            .expect("session should save with external profile");

        dispatch(CliCommand::SearchOpenTop(SearchOpenTopOptions {
            engine: SearchEngine::Google,
            session_file: Some(session_file.clone()),
            limit: 1,
            headed: false,
        }))
        .expect("search-open-top should succeed");

        let result_session_file = derived_search_result_session_file(&session_file, 1);
        let result_session = load_browser_cli_session(&result_session_file)
            .expect("child session should load after open-top");
        assert_eq!(
            result_session.browser_profile_dir.as_deref(),
            Some(profile_dir.to_string_lossy().as_ref())
        );
        assert_eq!(result_session.browser_context_dir, None);

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: result_session_file.clone(),
        }))
        .expect("child session close should succeed");
        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("search session close should succeed");
        fs::remove_dir_all(&profile_dir).expect("external profile dir cleanup should succeed");
    }

    #[test]
    fn dispatches_fixture_open_with_policy() {
        let output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/static-docs/getting-started".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            main_only: false,
            session_file: None,
        }))
        .expect("open should succeed");

        assert_eq!(output["status"], "succeeded");
        assert_eq!(output["policy"]["decision"], "allow");
        assert_eq!(output["payloadType"], "snapshot-document");
    }

    #[test]
    fn dispatches_hostile_policy_command() {
        let output = dispatch(CliCommand::Policy(TargetOptions {
            target: "fixture://research/hostile/fake-system-message".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            main_only: false,
            session_file: None,
        }))
        .expect("policy command should succeed");

        assert_eq!(output["policy"]["decision"], "block");
        assert_eq!(output["policy"]["riskClass"], "blocked");
    }

    #[test]
    fn dispatches_replay_command() {
        let output = dispatch(CliCommand::Replay {
            scenario: "read-only-pricing".to_string(),
        })
        .expect("replay should succeed");

        assert_eq!(output["snapshotCount"], 2);
        assert_eq!(output["evidenceReportCount"], 1);
    }

    #[test]
    fn dispatches_memory_summary_for_fifty_actions() {
        let output = dispatch(CliCommand::MemorySummary { steps: 50 })
            .expect("memory summary should succeed");

        assert_eq!(output["requestedActions"], 50);
        assert_eq!(output["memorySummary"]["turnCount"], 50);
        assert!(
            output["memorySummary"]["maxWorkingSetSize"]
                .as_u64()
                .expect("working set size should be numeric")
                <= 6
        );
    }

    #[test]
    fn parses_open_command_with_browser_flags() {
        let command = parse_command(&[
            "open".to_string(),
            "fixture://research/static-docs/getting-started".to_string(),
            "--browser".to_string(),
            "--headed".to_string(),
        ])
        .expect("open command should parse");

        assert_eq!(
            command,
            CliCommand::Open(TargetOptions {
                target: "fixture://research/static-docs/getting-started".to_string(),
                budget: DEFAULT_REQUESTED_TOKENS,
                source_risk: None,
                source_label: None,
                allowlisted_domains: Vec::new(),
                browser: true,
                headed: true,
                main_only: false,
                session_file: None,
            })
        );
    }

    #[test]
    fn parses_open_command_with_custom_budget() {
        let command = parse_command(&[
            "open".to_string(),
            "fixture://research/static-docs/getting-started".to_string(),
            "--budget".to_string(),
            "2048".to_string(),
        ])
        .expect("open command with budget should parse");

        assert_eq!(
            command,
            CliCommand::Open(TargetOptions {
                target: "fixture://research/static-docs/getting-started".to_string(),
                budget: 2048,
                source_risk: None,
                source_label: None,
                allowlisted_domains: Vec::new(),
                browser: false,
                headed: false,
                main_only: false,
                session_file: None,
            })
        );
    }

    #[test]
    fn parses_read_view_command_with_main_only() {
        let command = parse_command(&[
            "read-view".to_string(),
            "https://www.iana.org/help/example-domains".to_string(),
            "--main-only".to_string(),
        ])
        .expect("read-view command should parse");

        assert_eq!(
            command,
            CliCommand::ReadView(TargetOptions {
                target: "https://www.iana.org/help/example-domains".to_string(),
                budget: DEFAULT_REQUESTED_TOKENS,
                source_risk: None,
                source_label: None,
                allowlisted_domains: Vec::new(),
                browser: false,
                headed: false,
                main_only: true,
                session_file: None,
            })
        );
    }

    #[test]
    fn parses_session_read_command_with_main_only() {
        let command = parse_command(&[
            "session-read".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--main-only".to_string(),
        ])
        .expect("session-read command should parse");

        assert_eq!(
            command,
            CliCommand::SessionRead(SessionReadOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                main_only: true,
            })
        );
    }

    #[test]
    fn parses_click_command() {
        let command = parse_command(&[
            "click".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--ref".to_string(),
            "rmain:button:continue".to_string(),
            "--headed".to_string(),
        ])
        .expect("click command should parse");

        assert_eq!(
            command,
            CliCommand::Click(ClickOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                target_ref: "rmain:button:continue".to_string(),
                headed: true,
                ack_risks: Vec::new(),
            })
        );
    }

    #[test]
    fn parses_refresh_command() {
        let command = parse_command(&[
            "refresh".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
        ])
        .expect("refresh command should parse");

        assert_eq!(
            command,
            CliCommand::SessionRefresh(SessionRefreshOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                headed: false,
            })
        );
    }

    #[test]
    fn parses_checkpoint_command() {
        let command = parse_command(&[
            "checkpoint".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
        ])
        .expect("checkpoint command should parse");

        assert_eq!(
            command,
            CliCommand::SessionCheckpoint(SessionFileOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
            })
        );
    }

    #[test]
    fn parses_approve_command() {
        let command = parse_command(&[
            "approve".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--risk".to_string(),
            "mfa".to_string(),
            "--risk".to_string(),
            "auth".to_string(),
        ])
        .expect("approve command should parse");

        assert_eq!(
            command,
            CliCommand::Approve(ApproveOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
            })
        );
    }

    #[test]
    fn parses_set_profile_command() {
        let command = parse_command(&[
            "set-profile".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--profile".to_string(),
            "interactive-supervised-auth".to_string(),
        ])
        .expect("set-profile command should parse");

        assert_eq!(
            command,
            CliCommand::SetProfile(SessionProfileSetOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                profile: PolicyProfile::InteractiveSupervisedAuth,
            })
        );
    }

    #[test]
    fn parses_telemetry_recent_command() {
        let command = parse_command(&[
            "telemetry-recent".to_string(),
            "--limit".to_string(),
            "7".to_string(),
        ])
        .expect("telemetry-recent command should parse");

        assert_eq!(
            command,
            CliCommand::TelemetryRecent(TelemetryRecentOptions { limit: 7 })
        );
    }

    #[test]
    fn parses_click_command_with_ack_risk() {
        let command = parse_command(&[
            "click".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--ref".to_string(),
            "rmain:button:continue".to_string(),
            "--ack-risk".to_string(),
            "challenge".to_string(),
            "--ack-risk".to_string(),
            "auth".to_string(),
        ])
        .expect("click command with ack risks should parse");

        assert_eq!(
            command,
            CliCommand::Click(ClickOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                target_ref: "rmain:button:continue".to_string(),
                headed: false,
                ack_risks: vec![AckRisk::Challenge, AckRisk::Auth],
            })
        );
    }

    #[test]
    fn parses_type_command_with_sensitive_flag() {
        let command = parse_command(&[
            "type".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--ref".to_string(),
            "rmain:input:password".to_string(),
            "--value".to_string(),
            "hunter2".to_string(),
            "--sensitive".to_string(),
        ])
        .expect("type command should parse");

        assert_eq!(
            command,
            CliCommand::Type(TypeOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                target_ref: "rmain:input:password".to_string(),
                value: "hunter2".to_string(),
                headed: false,
                sensitive: true,
                ack_risks: Vec::new(),
            })
        );
    }

    #[test]
    fn parses_submit_command() {
        let command = parse_command(&[
            "submit".to_string(),
            "--session-file".to_string(),
            "/tmp/test-session.json".to_string(),
            "--ref".to_string(),
            "rmain:form:sign-in".to_string(),
        ])
        .expect("submit command should parse");

        assert_eq!(
            command,
            CliCommand::Submit(SubmitOptions {
                session_file: PathBuf::from("/tmp/test-session.json"),
                target_ref: "rmain:form:sign-in".to_string(),
                headed: false,
                ack_risks: Vec::new(),
                extra_prefill: Vec::new(),
            })
        );
    }

    #[test]
    fn dispatches_browser_backed_fixture_open() {
        let output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/static-docs/getting-started".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: None,
        }))
        .expect("browser-backed open should succeed");

        assert_eq!(output["status"], "succeeded");
        assert_eq!(output["output"]["source"]["sourceType"], "playwright");
        assert_eq!(output["policy"]["decision"], "allow");
    }

    #[test]
    fn dispatches_browser_backed_extract() {
        let output = dispatch(CliCommand::Extract(ExtractOptions {
            target: "fixture://research/citation-heavy/pricing".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            session_file: None,
            claims: vec!["The Starter plan costs $29 per month.".to_string()],
            verifier_command: None,
        }))
        .expect("browser-backed extract should succeed");

        assert_eq!(
            output["open"]["output"]["source"]["sourceType"],
            "playwright"
        );
        assert_eq!(output["extract"]["status"], "succeeded");
        assert_eq!(
            output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
            "The Starter plan costs $29 per month."
        );
    }

    #[test]
    fn attaches_verifier_outcomes_to_extract_results() {
        let output = dispatch(CliCommand::Extract(ExtractOptions {
            target: "fixture://research/citation-heavy/pricing".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            session_file: None,
            claims: vec!["The Starter plan costs $29 per month.".to_string()],
            verifier_command: Some(
                "printf '{\"outcomes\":[{\"claimId\":\"c1\",\"verdict\":\"verified\",\"verifierScore\":0.88,\"notes\":\"checked against source\"}]}'"
                    .to_string(),
            ),
        }))
        .expect("extract with verifier should succeed");

        assert_eq!(
            output["extract"]["output"]["verification"]["outcomes"][0]["verdict"],
            "verified"
        );
        assert_eq!(
            output["extract"]["output"]["verification"]["outcomes"][0]["verifierScore"],
            0.88
        );
        assert_eq!(
            output["extract"]["output"]["claimOutcomes"][0]["verdict"],
            "evidence-supported"
        );
    }

    #[test]
    fn verifier_can_demote_supported_claims_into_needs_more_browsing() {
        let output = dispatch(CliCommand::Extract(ExtractOptions {
            target: "fixture://research/citation-heavy/pricing".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            session_file: None,
            claims: vec!["The Starter plan costs $29 per month.".to_string()],
            verifier_command: Some(
                "printf '{\"outcomes\":[{\"claimId\":\"c1\",\"verdict\":\"needs-more-browsing\",\"verifierScore\":0.31,\"notes\":\"open a more specific pricing table before answering\"}]}'"
                    .to_string(),
            ),
        }))
        .expect("extract with demoting verifier should succeed");

        assert_eq!(
            output["extract"]["output"]["evidenceSupportedClaims"]
                .as_array()
                .expect("supported claims should be present")
                .len(),
            0
        );
        assert_eq!(
            output["extract"]["output"]["needsMoreBrowsingClaims"][0]["statement"],
            "The Starter plan costs $29 per month."
        );
        assert_eq!(
            output["extract"]["output"]["claimOutcomes"][0]["verificationVerdict"],
            "needs-more-browsing"
        );
    }

    #[test]
    fn dispatches_browser_backed_hostile_policy() {
        let output = dispatch(CliCommand::Policy(TargetOptions {
            target: "fixture://research/hostile/fake-system-message".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: None,
        }))
        .expect("browser-backed policy should succeed");

        assert_eq!(output["policy"]["decision"], "block");
        assert_eq!(output["policy"]["riskClass"], "blocked");
    }

    #[test]
    fn persists_browser_session_and_reads_current_snapshot() {
        let session_file = temp_session_path("session-open");
        let output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-pagination".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        assert_eq!(output["status"], "succeeded");
        assert!(session_file.exists());

        let snapshot = dispatch(CliCommand::SessionSnapshot(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session snapshot should succeed");

        assert_eq!(snapshot["action"]["status"], "succeeded");
        assert_eq!(
            snapshot["action"]["output"]["blocks"][1]["text"],
            "Browser Pagination"
        );

        fs::remove_file(session_file).ok();
    }

    #[test]
    fn refreshes_browser_session_from_current_live_state() {
        let session_file = temp_session_path("session-refresh");
        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let refreshed = dispatch(CliCommand::SessionRefresh(SessionRefreshOptions {
            session_file: session_file.clone(),
            headed: false,
        }))
        .expect("refresh should succeed");

        assert_eq!(refreshed["action"]["status"], "succeeded");
        assert_eq!(refreshed["action"]["action"], "read");

        fs::remove_file(session_file).ok();
    }

    #[test]
    fn paginates_browser_session_and_updates_snapshot() {
        let session_file = temp_session_path("session-paginate");
        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-pagination".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let output = dispatch(CliCommand::Paginate(PaginateOptions {
            session_file: session_file.clone(),
            direction: PaginationDirection::Next,
            headed: false,
        }))
        .expect("paginate should succeed");

        assert_eq!(output["action"]["status"], "succeeded");
        assert_eq!(output["action"]["action"], "paginate");
        assert_eq!(output["action"]["output"]["adapter"]["page"], 2);
        assert!(output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Page 2"));

        fs::remove_file(session_file).ok();
    }

    #[test]
    fn preserves_browser_dom_state_across_paginate_actions() {
        let session_file = temp_session_path("session-paginate-twice");
        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-pagination".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        dispatch(CliCommand::Paginate(PaginateOptions {
            session_file: session_file.clone(),
            direction: PaginationDirection::Next,
            headed: false,
        }))
        .expect("first paginate should succeed");

        let second_paginate = dispatch(CliCommand::Paginate(PaginateOptions {
            session_file: session_file.clone(),
            direction: PaginationDirection::Next,
            headed: false,
        }))
        .expect_err("second paginate should fail after the next button disappears");

        assert!(
            second_paginate
                .to_string()
                .contains("No next pagination target was found."),
            "unexpected error: {second_paginate}"
        );

        fs::remove_file(session_file).ok();
    }

    #[test]
    fn follows_browser_session_and_can_extract_from_persisted_state() {
        let session_file = temp_session_path("session-follow");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let follow_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "link")
            .and_then(|block| block["ref"].as_str())
            .expect("link ref should exist")
            .to_string();

        let follow_output = dispatch(CliCommand::Follow(FollowOptions {
            session_file: session_file.clone(),
            target_ref: follow_ref,
            headed: false,
        }))
        .expect("follow should succeed");

        assert_eq!(follow_output["action"]["status"], "succeeded");
        assert_eq!(follow_output["action"]["action"], "follow");
        assert_eq!(follow_output["result"]["status"], "succeeded");
        assert!(follow_output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Advanced guide opened for the next research step."));

        let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
            session_file: session_file.clone(),
            claims: vec!["Advanced guide opened for the next research step.".to_string()],
            verifier_command: None,
        }))
        .expect("session extract should succeed");

        assert_eq!(extract_output["extract"]["status"], "succeeded");
        assert_eq!(extract_output["result"]["status"], "succeeded");
        assert_eq!(
            extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
            "Advanced guide opened for the next research step."
        );

        let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session close should succeed");
        assert_eq!(close_output["removed"], true);
    }

    #[test]
    fn preserves_requested_budget_across_browser_follow_actions() {
        let session_file = temp_session_path("session-follow-budget");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: 64,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let follow_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "link")
            .and_then(|block| block["ref"].as_str())
            .expect("link ref should exist")
            .to_string();

        let follow_output = dispatch(CliCommand::Follow(FollowOptions {
            session_file: session_file.clone(),
            target_ref: follow_ref,
            headed: false,
        }))
        .expect("follow should succeed");

        assert_eq!(
            follow_output["action"]["output"]["snapshot"]["budget"]["requestedTokens"],
            64
        );

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn follows_duplicate_browser_link_using_stable_ref_ordinal_hint() {
        let session_file = temp_session_path("session-follow-duplicate");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow-duplicate".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let follow_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .filter(|block| block["kind"] == "link")
            .find(|block| {
                block["ref"]
                    .as_str()
                    .expect("ref should be present")
                    .ends_with(":2")
            })
            .and_then(|block| block["ref"].as_str())
            .expect("second link ref should exist")
            .to_string();

        let follow_output = dispatch(CliCommand::Follow(FollowOptions {
            session_file: session_file.clone(),
            target_ref: follow_ref,
            headed: false,
        }))
        .expect("follow should succeed");

        assert_eq!(follow_output["action"]["status"], "succeeded");
        assert!(follow_output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Current docs opened for the research step."));

        let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session close should succeed");
        assert_eq!(close_output["removed"], true);
    }

    #[test]
    fn expands_browser_session_and_can_extract_from_persisted_state() {
        let session_file = temp_session_path("session-expand");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-expand".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let expand_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "button")
            .and_then(|block| block["ref"].as_str())
            .expect("button ref should exist")
            .to_string();

        let expand_output = dispatch(CliCommand::Expand(ExpandOptions {
            session_file: session_file.clone(),
            target_ref: expand_ref,
            headed: false,
        }))
        .expect("expand should succeed");

        assert_eq!(expand_output["action"]["status"], "succeeded");
        assert!(expand_output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Expanded details confirm"));

        let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
            session_file: session_file.clone(),
            claims: vec![
                "Expanded details confirm that the runtime can reveal collapsed notes.".to_string(),
            ],
            verifier_command: None,
        }))
        .expect("session extract should succeed");

        assert_eq!(extract_output["extract"]["status"], "succeeded");
        assert_eq!(
            extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
            "Expanded details confirm that the runtime can reveal collapsed notes."
        );

        let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session close should succeed");
        assert_eq!(close_output["removed"], true);
    }

    #[test]
    fn types_into_browser_session_and_marks_session_interactive() {
        let session_file = temp_session_path("session-type");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-login-form".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let email_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| {
                block["kind"] == "input"
                    && block["text"]
                        .as_str()
                        .expect("input text should exist")
                        .contains("agent@example.com")
            })
            .and_then(|block| block["ref"].as_str())
            .expect("email input ref should exist")
            .to_string();

        let type_output = dispatch(CliCommand::Type(TypeOptions {
            session_file: session_file.clone(),
            target_ref: email_ref,
            value: "agent@example.com".to_string(),
            headed: false,
            sensitive: false,
            ack_risks: Vec::new(),
        }))
        .expect("type should succeed");

        assert_eq!(type_output["action"]["status"], "succeeded");
        assert_eq!(type_output["action"]["action"], "type");
        assert_eq!(type_output["sessionState"]["mode"], "interactive");
        assert_eq!(
            type_output["sessionState"]["policyProfile"],
            "interactive-review"
        );
        assert_eq!(
            type_output["action"]["output"]["adapter"]["typedLength"],
            17
        );
        assert!(type_output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("agent@example.com"));

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn rejects_sensitive_type_without_explicit_opt_in() {
        let session_file = temp_session_path("session-type-sensitive");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-login-form".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let password_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| {
                block["kind"] == "input"
                    && block["text"]
                        .as_str()
                        .expect("input text should exist")
                        .contains("password")
            })
            .and_then(|block| block["ref"].as_str())
            .expect("password input ref should exist")
            .to_string();

        let type_output = dispatch(CliCommand::Type(TypeOptions {
            session_file: session_file.clone(),
            target_ref: password_ref,
            value: "hunter2".to_string(),
            headed: false,
            sensitive: false,
            ack_risks: Vec::new(),
        }))
        .expect("type command should return a rejection");

        assert_eq!(type_output["action"]["status"], "rejected");
        assert_eq!(type_output["action"]["failureKind"], "policy-blocked");

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn clicks_browser_session_button_after_interactive_typing() {
        let session_file = temp_session_path("session-click");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-login-form".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let email_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| {
                block["kind"] == "input"
                    && block["text"]
                        .as_str()
                        .expect("input text should exist")
                        .contains("agent@example.com")
            })
            .and_then(|block| block["ref"].as_str())
            .expect("email input ref should exist")
            .to_string();
        let button_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "button")
            .and_then(|block| block["ref"].as_str())
            .expect("button ref should exist")
            .to_string();

        dispatch(CliCommand::Type(TypeOptions {
            session_file: session_file.clone(),
            target_ref: email_ref,
            value: "agent@example.com".to_string(),
            headed: false,
            sensitive: false,
            ack_risks: Vec::new(),
        }))
        .expect("type should succeed");

        let click_output = dispatch(CliCommand::Click(ClickOptions {
            session_file: session_file.clone(),
            target_ref: button_ref,
            headed: false,
            ack_risks: vec![AckRisk::Auth],
        }))
        .expect("click should succeed");

        assert_eq!(click_output["action"]["status"], "succeeded");
        assert_eq!(click_output["action"]["action"], "click");
        assert!(click_output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Signed in draft session ready for review."));

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn submits_browser_session_form_after_interactive_typing() {
        let session_file = temp_session_path("session-submit");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-login-form".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let email_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| {
                block["kind"] == "input"
                    && block["text"]
                        .as_str()
                        .expect("input text should exist")
                        .contains("agent@example.com")
            })
            .and_then(|block| block["ref"].as_str())
            .expect("email input ref should exist")
            .to_string();
        let form_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "form")
            .and_then(|block| block["ref"].as_str())
            .expect("form ref should exist")
            .to_string();

        dispatch(CliCommand::Type(TypeOptions {
            session_file: session_file.clone(),
            target_ref: email_ref,
            value: "agent@example.com".to_string(),
            headed: false,
            sensitive: false,
            ack_risks: Vec::new(),
        }))
        .expect("type should succeed");

        let submit_output = dispatch(CliCommand::Submit(SubmitOptions {
            session_file: session_file.clone(),
            target_ref: form_ref,
            headed: false,
            ack_risks: Vec::new(),
            extra_prefill: Vec::new(),
        }))
        .expect("submit should succeed");

        assert_eq!(submit_output["action"]["status"], "succeeded");
        assert_eq!(submit_output["action"]["action"], "submit");
        assert!(submit_output["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Signed in draft session ready for review."));

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn rejects_mfa_submit_without_ack_and_allows_it_with_ack() {
        let session_file = temp_session_path("session-mfa");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-mfa-challenge".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let otp_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "input")
            .and_then(|block| block["ref"].as_str())
            .expect("otp ref should exist")
            .to_string();
        let form_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "form")
            .and_then(|block| block["ref"].as_str())
            .expect("form ref should exist")
            .to_string();

        let blocked = dispatch(CliCommand::Submit(SubmitOptions {
            session_file: session_file.clone(),
            target_ref: form_ref.clone(),
            headed: false,
            ack_risks: Vec::new(),
            extra_prefill: Vec::new(),
        }))
        .expect("submit should return a rejection");
        assert_eq!(blocked["action"]["status"], "rejected");

        dispatch(CliCommand::Type(TypeOptions {
            session_file: session_file.clone(),
            target_ref: otp_ref,
            value: "123456".to_string(),
            headed: false,
            sensitive: true,
            ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
        }))
        .expect("sensitive MFA type should succeed");

        let approved = dispatch(CliCommand::Submit(SubmitOptions {
            session_file: session_file.clone(),
            target_ref: form_ref,
            headed: false,
            ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
            extra_prefill: Vec::new(),
        }))
        .expect("approved submit should succeed");
        assert_eq!(approved["action"]["status"], "succeeded");
        assert!(approved["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Verification code accepted for supervised review."));

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn checkpoint_and_approve_enable_supervised_session_without_repeating_ack_flags() {
        let session_file = temp_session_path("session-checkpoint-approve");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-mfa-challenge".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let otp_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "input")
            .and_then(|block| block["ref"].as_str())
            .expect("otp ref should exist")
            .to_string();
        let form_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "form")
            .and_then(|block| block["ref"].as_str())
            .expect("form ref should exist")
            .to_string();

        let checkpoint = dispatch(CliCommand::SessionCheckpoint(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("checkpoint should succeed");
        assert!(checkpoint["checkpoint"]["requiredAckRisks"]
            .as_array()
            .expect("required risks should be an array")
            .iter()
            .any(|risk| risk == "mfa"));
        assert!(checkpoint["checkpoint"]["requiredAckRisks"]
            .as_array()
            .expect("required risks should be an array")
            .iter()
            .any(|risk| risk == "auth"));
        assert_eq!(
            checkpoint["checkpoint"]["recommendedPolicyProfile"],
            "interactive-supervised-auth"
        );
        assert_eq!(
            checkpoint["checkpoint"]["playbook"]["provider"],
            "generic-auth"
        );

        let approval = dispatch(CliCommand::Approve(ApproveOptions {
            session_file: session_file.clone(),
            ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
        }))
        .expect("approve should succeed");
        assert!(approval["approvedRisks"]
            .as_array()
            .expect("approved risks should be an array")
            .iter()
            .any(|risk| risk == "mfa"));
        assert_eq!(approval["policyProfile"], "interactive-supervised-auth");

        dispatch(CliCommand::Type(TypeOptions {
            session_file: session_file.clone(),
            target_ref: otp_ref,
            value: "123456".to_string(),
            headed: false,
            sensitive: true,
            ack_risks: Vec::new(),
        }))
        .expect("approved MFA type should succeed without inline ack");

        let approved = dispatch(CliCommand::Submit(SubmitOptions {
            session_file: session_file.clone(),
            target_ref: form_ref,
            headed: false,
            ack_risks: Vec::new(),
            extra_prefill: Vec::new(),
        }))
        .expect("approved submit should succeed without inline ack");
        assert_eq!(approved["action"]["status"], "succeeded");

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn rejects_high_risk_submit_without_ack_and_allows_it_with_ack() {
        let session_file = temp_session_path("session-high-risk");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-high-risk-checkout".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: vec!["research".to_string()],
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let form_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "form")
            .and_then(|block| block["ref"].as_str())
            .expect("form ref should exist")
            .to_string();

        let blocked = dispatch(CliCommand::Submit(SubmitOptions {
            session_file: session_file.clone(),
            target_ref: form_ref.clone(),
            headed: false,
            ack_risks: Vec::new(),
            extra_prefill: Vec::new(),
        }))
        .expect("submit should return a rejection");
        assert_eq!(blocked["action"]["status"], "rejected");

        let approved = dispatch(CliCommand::Submit(SubmitOptions {
            session_file: session_file.clone(),
            target_ref: form_ref,
            headed: false,
            ack_risks: vec![AckRisk::HighRiskWrite],
            extra_prefill: Vec::new(),
        }))
        .expect("approved submit should succeed");
        assert_eq!(approved["action"]["status"], "succeeded");
        assert!(approved["action"]["output"]["adapter"]["visibleText"]
            .as_str()
            .expect("visible text should be present")
            .contains("Purchase confirmation staged for supervised review."));

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn dispatches_compact_view_for_fixture() {
        let output = dispatch(CliCommand::CompactView(TargetOptions {
            target: "fixture://research/static-docs/getting-started".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            main_only: false,
            session_file: None,
        }))
        .expect("compact view should succeed");

        assert_eq!(
            output["sourceUrl"],
            "fixture://research/static-docs/getting-started"
        );
        assert!(output["compactText"]
            .as_str()
            .expect("compact text should exist")
            .contains("Getting Started"));
        assert!(output["readingCompactText"]
            .as_str()
            .expect("reading compact text should exist")
            .contains("Getting Started"));
        assert!(output["navigationCompactText"]
            .as_str()
            .expect("navigation compact text should exist")
            .contains("Docs"));
        assert_ne!(
            output["compactText"], output["navigationCompactText"],
            "compact and navigation outputs should remain distinct surfaces",
        );
        assert!(
            output["lineCount"]
                .as_u64()
                .expect("line count should be numeric")
                > 0
        );
    }

    #[test]
    fn dispatches_session_compact_for_browser_session() {
        let session_file = temp_session_path("session-compact");
        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let output = dispatch(CliCommand::SessionCompact(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session compact should succeed");

        assert_eq!(output["sessionFile"], session_file.display().to_string());
        assert!(output["compactText"]
            .as_str()
            .expect("compact text should exist")
            .contains("Browser Follow"));
        assert!(output["readingCompactText"]
            .as_str()
            .expect("reading compact text should exist")
            .contains("Browser Follow"));
        assert!(output["navigationCompactText"]
            .as_str()
            .expect("navigation compact text should exist")
            .contains("Advanced guide"));
        assert_ne!(
            output["compactText"],
            output["navigationCompactText"],
            "session compact output should keep the navigation slice separate from the primary compact surface",
        );

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn replays_browser_trace_into_new_browser_session() {
        let session_file = temp_session_path("browser-replay");
        let open_output = dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");
        let follow_ref = open_output["output"]["blocks"]
            .as_array()
            .expect("blocks should exist")
            .iter()
            .find(|block| block["kind"] == "link")
            .and_then(|block| block["ref"].as_str())
            .expect("link ref should exist")
            .to_string();

        dispatch(CliCommand::Follow(FollowOptions {
            session_file: session_file.clone(),
            target_ref: follow_ref,
            headed: false,
        }))
        .expect("follow should succeed");

        let replay_output = dispatch(CliCommand::BrowserReplay(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("browser replay should succeed");

        assert_eq!(replay_output["replayedActions"], 1);
        assert!(replay_output["compactText"]
            .as_str()
            .expect("compact text should exist")
            .contains("Advanced opened"));
        assert_eq!(
            replay_output["sessionState"]["currentUrl"],
            "fixture://research/navigation/browser-follow"
        );

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file,
        }))
        .expect("session close should succeed");
    }

    #[test]
    fn session_close_removes_browser_context_directory() {
        let session_file = temp_session_path("browser-context-cleanup");
        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let context_dir = browser_context_dir_for_session_file(&session_file);
        assert!(context_dir.exists(), "browser context dir should exist");

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session close should succeed");

        assert!(
            !context_dir.exists(),
            "browser context dir should be removed on close"
        );
    }

    #[test]
    fn session_close_preserves_external_profile_directory() {
        let session_file = temp_session_path("browser-profile-preserve");
        let profile_dir = std::env::temp_dir().join(format!(
            "touch-browser-preserved-profile-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be monotonic")
                .as_nanos()
        ));
        fs::create_dir_all(&profile_dir).expect("external profile dir should exist");

        dispatch(CliCommand::Open(TargetOptions {
            target: "fixture://research/navigation/browser-follow".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: false,
            main_only: false,
            session_file: Some(session_file.clone()),
        }))
        .expect("browser-backed open should persist session");

        let mut persisted =
            load_browser_cli_session(&session_file).expect("session should load after open");
        if let Some(context_dir) = persisted.browser_context_dir.as_ref() {
            let context_path = PathBuf::from(context_dir);
            if context_path.exists() {
                fs::remove_dir_all(context_path).expect("managed context dir should clean up");
            }
        }
        persisted.browser_context_dir = None;
        persisted.browser_profile_dir = Some(profile_dir.display().to_string());
        save_browser_cli_session(&session_file, &persisted)
            .expect("session should save external profile state");

        dispatch(CliCommand::SessionClose(SessionFileOptions {
            session_file: session_file.clone(),
        }))
        .expect("session close should succeed");

        assert!(
            profile_dir.exists(),
            "external profile dir should not be removed on close"
        );

        fs::remove_dir_all(&profile_dir).expect("external profile dir cleanup should succeed");
    }
}
