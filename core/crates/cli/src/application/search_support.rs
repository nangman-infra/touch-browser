use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use kuchiki::{parse_html, traits::*, NodeRef};
use serde_json::Value;
use url::{form_urlencoded, Url};

use super::deps::{
    repo_root, CliError, SearchActionActor, SearchActionHint, SearchEngine, SearchReport,
    SearchReportStatus, SearchResultItem, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole,
    SnapshotDocument, CONTRACT_VERSION,
};

pub(crate) fn build_search_url(engine: SearchEngine, query: &str) -> Result<String, CliError> {
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

pub(crate) fn default_search_session_file(engine: SearchEngine) -> PathBuf {
    default_search_output_dir().join(format!(
        "{}.search-session.json",
        search_engine_slug(engine)
    ))
}

pub(crate) fn default_search_output_dir() -> PathBuf {
    repo_root().join("output/browser-search")
}

pub(crate) fn resolve_search_session_file(
    session_file: Option<&PathBuf>,
    engine: SearchEngine,
) -> PathBuf {
    session_file
        .cloned()
        .unwrap_or_else(|| default_search_session_file(engine))
}

pub(crate) fn resolve_latest_search_session_file(
    session_file: Option<&PathBuf>,
    engine: Option<SearchEngine>,
) -> Result<PathBuf, CliError> {
    match session_file {
        Some(path) => Ok(path.clone()),
        None => {
            if let Some(engine) = engine {
                let engine_default = default_search_session_file(engine);
                if engine_default.is_file() {
                    return Ok(engine_default);
                }
                return Err(CliError::Usage(format!(
                    "No persisted {} search session was found. Run `touch-browser search ... --engine {}` first or pass `--session-file <path>`.",
                    search_engine_slug(engine),
                    search_engine_slug(engine)
                )));
            }
            latest_search_session_file_in(&default_search_output_dir())?.ok_or_else(|| {
                CliError::Usage(
                    "No persisted search session was found. Run `touch-browser search ...` first or pass `--session-file <path>`.".to_string(),
                )
            })
        }
    }
}

pub(crate) fn latest_search_session_file_in(
    search_output_dir: &Path,
) -> Result<Option<PathBuf>, CliError> {
    if !search_output_dir.exists() {
        return Ok(None);
    }

    let mut candidates = fs::read_dir(search_output_dir)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if !path.is_file() || path.extension().and_then(|value| value.to_str()) != Some("json")
            {
                return None;
            }
            let modified = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())?;
            Some((modified, path))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| right.0.cmp(&left.0));

    Ok(candidates.into_iter().map(|(_, path)| path).next())
}

pub(crate) fn derived_search_result_session_file(
    search_session_file: &Path,
    rank: usize,
) -> PathBuf {
    let parent = search_session_file
        .parent()
        .unwrap_or_else(|| Path::new("/tmp"));
    let stem = search_session_file
        .file_stem()
        .map(|value| value.to_string_lossy().to_string())
        .unwrap_or_else(|| "touch-browser-search".to_string());
    parent.join(format!("{stem}.rank-{rank}.json"))
}

pub(crate) fn is_search_results_target(target: &str) -> bool {
    let Ok(url) = Url::parse(target) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let path = url.path();
    (is_google_host(host) || is_brave_host(host)) && path == "/search"
}

pub(crate) fn build_search_report(
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
        let identity = search_result_identity_key(&url);
        if !seen_urls.insert(identity) {
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
        let identity = search_result_identity_key(&url);
        if !seen_urls.insert(identity) {
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
        let snippet = search_result_snippet_from_anchor(anchor.as_node(), &title);

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
        .map(|result| search_result_identity_key(&result.url))
        .collect::<BTreeSet<_>>();
    for candidate in additional {
        if !seen.insert(search_result_identity_key(&candidate.url)) {
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
                            let nested = Url::parse(&value).ok()?;
                            return canonicalize_search_result_url(nested);
                        }
                    }
                }
            }
            None
        }
        SearchEngine::Brave if is_brave_host(host) => None,
        _ => matches!(resolved.scheme(), "http" | "https")
            .then_some(resolved)
            .and_then(canonicalize_search_result_url),
    }
}

fn canonicalize_search_result_url(mut resolved: Url) -> Option<String> {
    resolved.set_fragment(None);
    let host = resolved.host_str()?.to_ascii_lowercase();

    if matches!(
        host.as_str(),
        "youtube.com" | "www.youtube.com" | "m.youtube.com"
    ) {
        if resolved.path() == "/watch" {
            let video_id = resolved
                .query_pairs()
                .find(|(key, _)| key == "v")
                .map(|(_, value)| value.into_owned())?;
            let mut canonical = Url::parse("https://www.youtube.com/watch").ok()?;
            canonical.query_pairs_mut().append_pair("v", &video_id);
            return Some(canonical.to_string());
        }
        if resolved.path().starts_with("/shorts/") {
            resolved.set_query(None);
            return Some(resolved.to_string());
        }
    }

    if host == "youtu.be" {
        resolved.set_query(None);
        return Some(resolved.to_string());
    }

    Some(resolved.to_string())
}

fn search_result_identity_key(url: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(canonicalize_search_result_url)
        .unwrap_or_else(|| url.to_string())
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

pub(crate) fn search_engine_source_label(engine: SearchEngine) -> &'static str {
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
