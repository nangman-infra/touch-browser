use std::{
    collections::BTreeSet,
    env, fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

use kuchiki::{parse_html, traits::*, NodeRef};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::{form_urlencoded, Url};

use super::deps::{
    CliError, SearchActionActor, SearchActionHint, SearchEngine, SearchReport, SearchReportStatus,
    SearchResultItem, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotDocument,
    CONTRACT_VERSION,
};
use crate::interface::cli_support::data_root;

fn search_metadata_version() -> String {
    CONTRACT_VERSION.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct PreferredSearchEngineRecord {
    #[serde(default = "search_metadata_version")]
    version: String,
    engine: SearchEngine,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SearchProfileStateRecord {
    #[serde(default = "search_metadata_version")]
    pub(crate) version: String,
    pub(crate) engine: SearchEngine,
    pub(crate) profile_dir: String,
    pub(crate) last_success_at: Option<String>,
    pub(crate) last_challenge_at: Option<String>,
    pub(crate) last_manual_recovery_at: Option<String>,
    pub(crate) consecutive_challenges: usize,
}

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

fn default_search_session_file_in(search_output_dir: &Path, engine: SearchEngine) -> PathBuf {
    search_output_dir.join(format!(
        "{}.search-session.json",
        search_engine_slug(engine)
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn default_search_session_file(engine: SearchEngine) -> PathBuf {
    default_search_session_file_in(&default_search_output_dir(), engine)
}

pub(crate) fn default_search_output_dir() -> PathBuf {
    data_root().join("browser-search")
}

pub(crate) fn default_search_profile_root() -> PathBuf {
    default_search_output_dir().join("profiles")
}

pub(crate) fn default_search_profile_dir(engine: SearchEngine) -> PathBuf {
    default_search_profile_root().join(format!("{}-default", search_engine_slug(engine)))
}

fn source_checkout_root() -> Option<PathBuf> {
    if let Some(explicit_root) =
        env::var_os("TOUCH_BROWSER_REPO_ROOT").filter(|value| !value.is_empty())
    {
        return Some(canonical_or_raw(PathBuf::from(explicit_root)));
    }

    let manifest_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    manifest_root
        .exists()
        .then(|| canonical_or_raw(manifest_root))
}

fn legacy_search_output_dir() -> Option<PathBuf> {
    let legacy_dir = source_checkout_root()?.join("output/browser-search");
    let canonical_legacy = canonical_or_raw(legacy_dir);
    (canonical_legacy != default_search_output_dir()).then_some(canonical_legacy)
}

fn default_or_legacy_search_session_file_for(
    engine: SearchEngine,
    default_output_dir: &Path,
    legacy_output_dir: Option<&Path>,
) -> PathBuf {
    let default_session = default_search_session_file_in(default_output_dir, engine);
    if default_session.is_file() {
        return default_session;
    }

    legacy_output_dir
        .map(|dir| default_search_session_file_in(dir, engine))
        .filter(|path| path.is_file())
        .unwrap_or(default_session)
}

fn default_or_legacy_search_session_file(engine: SearchEngine) -> PathBuf {
    default_or_legacy_search_session_file_for(
        engine,
        &default_search_output_dir(),
        legacy_search_output_dir().as_deref(),
    )
}

fn preferred_search_engine_file_in(search_output_dir: &Path) -> PathBuf {
    search_output_dir.join("preferred-engine.json")
}

fn preferred_search_engine_file() -> PathBuf {
    preferred_search_engine_file_in(&default_search_output_dir())
}

fn search_profile_state_file_in(search_output_dir: &Path, engine: SearchEngine) -> PathBuf {
    search_output_dir.join(format!("{}.profile-state.json", search_engine_slug(engine)))
}

fn search_profile_state_file(engine: SearchEngine) -> PathBuf {
    search_profile_state_file_in(&default_search_output_dir(), engine)
}

fn infer_search_engine_from_session_file(path: &Path) -> Option<SearchEngine> {
    let file_name = path.file_name()?.to_str()?;
    if file_name.starts_with("google.search-session") {
        return Some(SearchEngine::Google);
    }
    if file_name.starts_with("brave.search-session") {
        return Some(SearchEngine::Brave);
    }
    None
}

pub(crate) fn resolve_search_session_file(
    session_file: Option<&PathBuf>,
    engine: SearchEngine,
) -> PathBuf {
    session_file
        .cloned()
        .unwrap_or_else(|| default_or_legacy_search_session_file(engine))
}

pub(crate) fn load_preferred_search_engine() -> Result<Option<SearchEngine>, CliError> {
    if let Some(engine) = load_preferred_search_engine_from(&preferred_search_engine_file())? {
        return Ok(Some(engine));
    }

    Ok(latest_search_session_file()?
        .as_deref()
        .and_then(infer_search_engine_from_session_file))
}

fn load_preferred_search_engine_from(
    metadata_file: &Path,
) -> Result<Option<SearchEngine>, CliError> {
    if !metadata_file.is_file() {
        return Ok(None);
    }

    let bytes = fs::read(metadata_file)?;
    let record = serde_json::from_slice::<PreferredSearchEngineRecord>(&bytes).ok();
    Ok(record.map(|record| record.engine))
}

pub(crate) fn save_preferred_search_engine(engine: SearchEngine) -> Result<(), CliError> {
    save_preferred_search_engine_to(&preferred_search_engine_file(), engine)
}

fn save_preferred_search_engine_to(
    metadata_file: &Path,
    engine: SearchEngine,
) -> Result<(), CliError> {
    if let Some(parent) = metadata_file.parent() {
        fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_vec_pretty(&PreferredSearchEngineRecord {
        version: search_metadata_version(),
        engine,
    })
    .map_err(|error| CliError::Usage(error.to_string()))?;
    fs::write(metadata_file, payload)?;
    Ok(())
}

pub(crate) fn resolve_search_profile_dir(
    explicit_profile_dir: Option<&PathBuf>,
    engine: SearchEngine,
) -> PathBuf {
    explicit_profile_dir
        .cloned()
        .unwrap_or_else(|| default_search_profile_dir(engine))
}

#[allow(dead_code)]
pub(crate) fn load_search_profile_state(
    engine: SearchEngine,
) -> Result<Option<SearchProfileStateRecord>, CliError> {
    load_search_profile_state_from(&search_profile_state_file(engine))
}

fn load_search_profile_state_from(
    metadata_file: &Path,
) -> Result<Option<SearchProfileStateRecord>, CliError> {
    if !metadata_file.is_file() {
        return Ok(None);
    }
    let bytes = fs::read(metadata_file)?;
    let record = serde_json::from_slice::<SearchProfileStateRecord>(&bytes).ok();
    Ok(record)
}

pub(crate) fn record_search_profile_result(
    engine: SearchEngine,
    profile_dir: &Path,
    status: SearchReportStatus,
    headed: bool,
    timestamp: &str,
) -> Result<SearchProfileStateRecord, CliError> {
    let metadata_file = search_profile_state_file(engine);
    let mut record =
        load_search_profile_state_from(&metadata_file)?.unwrap_or(SearchProfileStateRecord {
            version: search_metadata_version(),
            engine,
            profile_dir: profile_dir.display().to_string(),
            last_success_at: None,
            last_challenge_at: None,
            last_manual_recovery_at: None,
            consecutive_challenges: 0,
        });
    record.version = search_metadata_version();
    record.engine = engine;
    record.profile_dir = profile_dir.display().to_string();

    match status {
        SearchReportStatus::Challenge => {
            record.last_challenge_at = Some(timestamp.to_string());
            record.consecutive_challenges += 1;
        }
        SearchReportStatus::Ready | SearchReportStatus::NoResults => {
            record.last_success_at = Some(timestamp.to_string());
            record.consecutive_challenges = 0;
            if headed {
                record.last_manual_recovery_at = Some(timestamp.to_string());
            }
        }
    }

    if let Some(parent) = metadata_file.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(
        &metadata_file,
        serde_json::to_vec_pretty(&record).map_err(|error| CliError::Usage(error.to_string()))?,
    )?;
    Ok(record)
}

pub(crate) fn resolve_latest_search_session_file(
    session_file: Option<&PathBuf>,
    engine: Option<SearchEngine>,
) -> Result<PathBuf, CliError> {
    match session_file {
        Some(path) => Ok(path.clone()),
        None => {
            if let Some(engine) = engine {
                let engine_default = default_or_legacy_search_session_file(engine);
                if engine_default.is_file() {
                    return Ok(engine_default);
                }
                return Err(CliError::Usage(format!(
                    "No persisted {} search session was found. Run `touch-browser search ... --engine {}` first or pass `--session-file <path>`.",
                    search_engine_slug(engine),
                    search_engine_slug(engine)
                )));
            }
            latest_search_session_file()?
                .ok_or_else(|| {
                    CliError::Usage(
                        "No persisted search session was found. Run `touch-browser search ...` first or pass `--session-file <path>`.".to_string(),
                    )
                })
        }
    }
}

fn latest_search_session_file() -> Result<Option<PathBuf>, CliError> {
    latest_search_session_file_for(
        &default_search_output_dir(),
        legacy_search_output_dir().as_deref(),
    )
}

fn latest_search_session_file_for(
    default_output_dir: &Path,
    legacy_output_dir: Option<&Path>,
) -> Result<Option<PathBuf>, CliError> {
    let default_latest = latest_search_session_file_in(default_output_dir)?;
    let legacy_latest = legacy_output_dir
        .map(latest_search_session_file_in)
        .transpose()?
        .flatten();
    newest_by_modified(default_latest, legacy_latest)
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
    candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.0));

    Ok(candidates.into_iter().map(|(_, path)| path).next())
}

fn newest_by_modified(
    left: Option<PathBuf>,
    right: Option<PathBuf>,
) -> Result<Option<PathBuf>, CliError> {
    match (left, right) {
        (Some(left), Some(right)) => {
            let left_modified = fs::metadata(&left)?.modified()?;
            let right_modified = fs::metadata(&right)?.modified()?;
            Ok(Some(if right_modified > left_modified {
                right
            } else {
                left
            }))
        }
        (Some(path), None) | (None, Some(path)) => Ok(Some(path)),
        (None, None) => Ok(None),
    }
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

pub(crate) fn should_use_browser_research_identity(target: &str) -> bool {
    let Ok(url) = Url::parse(target) else {
        return false;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return false;
    }

    let Some(host) = url.host_str() else {
        return false;
    };
    if host.eq_ignore_ascii_case("localhost") {
        return false;
    }
    if host
        .parse::<IpAddr>()
        .map(|address| address.is_loopback())
        .unwrap_or(false)
    {
        return false;
    }

    true
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
        recovery: None,
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
        let domain = url_domain(&url);
        let title = clean_search_result_title(&block.text, &domain);
        if title.len() < 6 {
            continue;
        }
        let snippet = collect_search_result_snippet(&snapshot.blocks, index);

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

        let domain = url_domain(&url);
        let title = clean_search_result_title(&anchor.text_contents(), &domain);
        if title.len() < 6 {
            continue;
        }
        if looks_like_search_nav_link(&title) {
            continue;
        }
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

fn clean_search_result_title(raw_title: &str, domain: &str) -> String {
    let mut title = collapse_whitespace(raw_title);
    if let Some((_, tail)) = title.rsplit_once('›') {
        let cleaned_tail = tail.trim();
        if !cleaned_tail.is_empty() {
            title = cleaned_tail.to_string();
        }
    }

    let bare_domain = domain.strip_prefix("www.").unwrap_or(domain);
    for prefix in [domain, bare_domain] {
        if let Some(rest) = title.strip_prefix(prefix) {
            let rest = rest.trim_start_matches([' ', '-', '|', ':', '›']).trim();
            if !rest.is_empty() {
                title = rest.to_string();
                break;
            }
        }
    }

    remove_leading_breadcrumb_slug(&title)
}

fn remove_leading_breadcrumb_slug(title: &str) -> String {
    let Some((first, rest)) = title.split_once(' ') else {
        return title.to_string();
    };
    let rest = rest.trim();
    if rest.is_empty() {
        return title.to_string();
    }

    let first_as_words = first.replace(['-', '_', '/'], " ");
    let first_normalized = first_as_words.to_ascii_lowercase();
    let rest_normalized = rest.to_ascii_lowercase();
    let looks_like_slug = first
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '-' | '_' | '/'));
    if looks_like_slug
        && (rest_normalized.starts_with(&first_normalized)
            || rest.chars().next().is_some_and(char::is_uppercase))
    {
        return rest.to_string();
    }

    title.to_string()
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

    canonicalize_known_docs_url(&mut resolved, &host);
    Some(resolved.to_string())
}

fn canonicalize_known_docs_url(resolved: &mut Url, host: &str) {
    if host == "developer.mozilla.org" {
        canonicalize_mdn_docs_url(resolved);
        strip_query_params(
            resolved,
            &[
                "retiredLocale",
                "redirectlocale",
                "utm_source",
                "utm_medium",
                "utm_campaign",
                "utm_term",
                "utm_content",
            ],
        );
    }

    if host == "developers.google.com" {
        strip_query_params(
            resolved,
            &[
                "hl",
                "authuser",
                "utm_source",
                "utm_medium",
                "utm_campaign",
                "utm_term",
                "utm_content",
            ],
        );
    }
}

fn canonicalize_mdn_docs_url(resolved: &mut Url) {
    let Some(segments) = resolved
        .path_segments()
        .map(|parts| parts.collect::<Vec<_>>())
    else {
        return;
    };

    let canonical_segments = if segments.first().copied() == Some("docs") {
        let mut rewritten = vec!["en-US".to_string(), "docs".to_string()];
        rewritten.extend(
            segments
                .iter()
                .skip(1)
                .map(|segment| (*segment).to_string()),
        );
        Some(rewritten)
    } else if segments.len() >= 2
        && is_locale_path_segment(segments[0])
        && segments[1].eq_ignore_ascii_case("docs")
    {
        let mut rewritten = vec!["en-US".to_string(), "docs".to_string()];
        rewritten.extend(
            segments
                .iter()
                .skip(2)
                .map(|segment| (*segment).to_string()),
        );
        Some(rewritten)
    } else {
        None
    };

    if let Some(segments) = canonical_segments {
        resolved.set_path(&format!("/{}", segments.join("/")));
    }
}

fn is_locale_path_segment(segment: &str) -> bool {
    let mut parts = segment.split('-');
    let Some(language) = parts.next() else {
        return false;
    };
    if !(language.len() == 2 || language.len() == 3)
        || !language
            .chars()
            .all(|character| character.is_ascii_alphabetic())
    {
        return false;
    }

    parts.all(|part| {
        (part.len() == 2 || part.len() == 4)
            && part
                .chars()
                .all(|character| character.is_ascii_alphanumeric())
    })
}

fn strip_query_params(resolved: &mut Url, blocked_keys: &[&str]) {
    let Some(_) = resolved.query() else {
        return;
    };

    let retained = resolved
        .query_pairs()
        .filter(|(key, _)| {
            let lowered = key.to_ascii_lowercase();
            !blocked_keys.iter().any(|blocked| lowered == *blocked) && !lowered.starts_with("utm_")
        })
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();

    if retained.is_empty() {
        resolved.set_query(None);
        return;
    }

    let mut serializer = form_urlencoded::Serializer::new(String::new());
    for (key, value) in retained {
        serializer.append_pair(&key, &value);
    }
    resolved.set_query(Some(&serializer.finish()));
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

fn query_authority_hints(query: &str) -> Vec<&'static str> {
    let lowered = query.to_ascii_lowercase();
    let mut hints = Vec::new();

    push_query_authority_hint_if(
        &mut hints,
        lowered.contains("nodejs") || lowered.contains("node.js") || lowered.contains("node:"),
        "nodejs.org",
    );
    push_query_authority_hint_if(&mut hints, lowered.contains("mdn"), "developer.mozilla.org");
    push_query_authority_hint_if(&mut hints, lowered.contains("iana"), "iana.org");
    push_query_authority_hint_if(&mut hints, lowered.contains("rfc"), "rfc-editor.org");
    push_query_authority_hint_if(&mut hints, lowered.contains("react"), "react.dev");
    push_query_authority_hint_if(
        &mut hints,
        lowered.contains("nextjs") || lowered.contains("next.js"),
        "nextjs.org",
    );
    push_query_authority_hint_if(&mut hints, lowered.contains("tailwind"), "tailwindcss.com");
    push_query_authority_hint_if(
        &mut hints,
        lowered.contains("material ui") || lowered.contains("mui"),
        "mui.com",
    );
    push_query_authority_hint_if(
        &mut hints,
        lowered.contains("whatwg"),
        "url.spec.whatwg.org",
    );

    hints
}

fn push_query_authority_hint_if(
    hints: &mut Vec<&'static str>,
    condition: bool,
    domain: &'static str,
) {
    if condition && !hints.contains(&domain) {
        hints.push(domain);
    }
}

fn domain_matches_hint(domain: &str, hint: &str) -> bool {
    let normalized_domain = domain.trim_start_matches("www.");
    let normalized_hint = hint.trim_start_matches("www.");
    normalized_domain == normalized_hint
        || normalized_domain.ends_with(&format!(".{normalized_hint}"))
}

fn query_prefers_authoritative_docs(query: &str) -> bool {
    let lowered = query.to_ascii_lowercase();
    [
        "api",
        "docs",
        "documentation",
        "reference",
        "official",
        "manual",
    ]
    .iter()
    .any(|keyword| lowered.contains(keyword))
}

fn authority_domain_bonus(query: &str, result: &SearchResultItem) -> f64 {
    let hints = query_authority_hints(query);
    if hints.is_empty() {
        return 0.0;
    }

    let domain = result.domain.to_ascii_lowercase();
    let prefers_authoritative_docs = query_prefers_authoritative_docs(query);
    if let Some(index) = hints
        .iter()
        .position(|hint| domain_matches_hint(&domain, hint))
    {
        let base_bonus = match index {
            0 => 0.45,
            1 => 0.08,
            2 => 0.04,
            _ => 0.02,
        };
        let secondary_authority_penalty = if index > 0 && prefers_authoritative_docs {
            0.12
        } else {
            0.0
        };
        return base_bonus - secondary_authority_penalty;
    }

    if result.official_likely {
        if prefers_authoritative_docs {
            -0.16
        } else {
            -0.12
        }
    } else {
        0.0
    }
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
    score += authority_domain_bonus(query, result);
    if detail_keywords.iter().any(|keyword| {
        lowered_title.contains(keyword)
            || lowered_url.contains(keyword)
            || lowered_snippet.contains(keyword)
    }) {
        score += if numeric_intent { 0.22 } else { 0.10 };
    }
    if lowered_url.contains("/en-us/docs/") {
        score += 0.04;
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
            engine: None,
            command: None,
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
            engine: None,
            command: None,
            can_auto_run: false,
            headed_required: false,
            result_ranks: Vec::new(),
        }];
    }

    let mut hints = vec![SearchActionHint {
        action: "open-top".to_string(),
        detail: "Open the highest-ranked candidate tabs first, then run read-view or extract on the most specific pages.".to_string(),
        actor: SearchActionActor::Ai,
        engine: None,
        command: None,
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
            engine: None,
            command: None,
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
            engine: None,
            command: None,
            can_auto_run: true,
            headed_required: false,
            result_ranks: recommended_ranks.to_vec(),
        });
    } else {
        hints.push(SearchActionHint {
            action: "read-view".to_string(),
            detail: "Use read-view on the most relevant tabs first, then run extract only after the scope looks right.".to_string(),
            actor: SearchActionActor::Ai,
            engine: None,
            command: None,
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

fn canonical_or_raw(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::Mutex,
        time::{SystemTime, UNIX_EPOCH},
    };

    use serde_json::Value;
    use url::Url;

    use super::{
        authority_domain_bonus, canonicalize_search_result_url, clean_search_result_title,
        default_or_legacy_search_session_file_for, default_search_output_dir,
        default_search_profile_dir, infer_search_engine_from_session_file,
        latest_search_session_file_for, latest_search_session_file_in,
        load_preferred_search_engine_from, load_search_profile_state_from,
        record_search_profile_result, save_preferred_search_engine_to, selection_score_for_result,
        should_use_browser_research_identity, SearchEngine, SearchReportStatus, SearchResultItem,
    };
    use crate::CONTRACT_VERSION;

    fn temporary_directory(prefix: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should move forward")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("touch-browser-{prefix}-{unique}"));
        fs::create_dir_all(&path).expect("temporary directory should exist");
        path
    }

    #[test]
    fn default_search_output_dir_respects_explicit_data_root() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let data_root = temporary_directory("search-data-root");
        let previous = std::env::var_os("TOUCH_BROWSER_DATA_ROOT");
        std::env::set_var("TOUCH_BROWSER_DATA_ROOT", &data_root);

        assert_eq!(
            default_search_output_dir(),
            data_root
                .canonicalize()
                .unwrap_or(data_root.clone())
                .join("browser-search")
        );

        restore_env("TOUCH_BROWSER_DATA_ROOT", previous);
    }

    #[test]
    fn resolve_search_session_file_prefers_legacy_session_when_new_default_is_missing() {
        let data_root = temporary_directory("search-data-root-new");
        let repo_root = temporary_directory("search-legacy-repo");
        let legacy_session = repo_root.join("output/browser-search/google.search-session.json");
        fs::create_dir_all(
            legacy_session
                .parent()
                .expect("legacy session parent should exist"),
        )
        .expect("legacy session parent should be created");
        fs::write(&legacy_session, "{}").expect("legacy session file should exist");

        assert_eq!(
            default_or_legacy_search_session_file_for(
                SearchEngine::Google,
                &data_root.join("browser-search"),
                Some(&repo_root.join("output/browser-search")),
            )
            .canonicalize()
            .unwrap_or_else(|_| {
                default_or_legacy_search_session_file_for(
                    SearchEngine::Google,
                    &data_root.join("browser-search"),
                    Some(&repo_root.join("output/browser-search")),
                )
            }),
            legacy_session
                .canonicalize()
                .unwrap_or_else(|_| legacy_session.clone())
        );
    }

    #[test]
    fn resolve_latest_search_session_file_checks_legacy_output_dir() {
        let data_root = temporary_directory("search-data-root-empty");
        let repo_root = temporary_directory("search-legacy-latest");
        let legacy_session = repo_root.join("output/browser-search/brave.search-session.json");
        fs::create_dir_all(
            legacy_session
                .parent()
                .expect("legacy session parent should exist"),
        )
        .expect("legacy session parent should be created");
        fs::write(&legacy_session, "{}").expect("legacy latest session file should exist");

        assert_eq!(
            latest_search_session_file_for(
                &data_root.join("browser-search"),
                Some(&repo_root.join("output/browser-search")),
            )
            .expect("legacy session should resolve")
            .map(|path| path.canonicalize().unwrap_or(path)),
            Some(
                legacy_session
                    .canonicalize()
                    .unwrap_or_else(|_| legacy_session.clone())
            )
        );
    }

    #[test]
    fn preferred_search_engine_round_trips_through_metadata_file() {
        let data_root = temporary_directory("search-preferred-engine");
        let metadata_file = data_root.join("browser-search/preferred-engine.json");

        assert_eq!(
            load_preferred_search_engine_from(&metadata_file)
                .expect("preferred engine should load"),
            None
        );
        save_preferred_search_engine_to(&metadata_file, SearchEngine::Brave)
            .expect("preferred engine metadata should save");
        assert_eq!(
            load_preferred_search_engine_from(&metadata_file)
                .expect("preferred engine should reload"),
            Some(SearchEngine::Brave)
        );
        let raw = fs::read_to_string(&metadata_file).expect("metadata file should be readable");
        let payload: Value = serde_json::from_str(&raw).expect("metadata should be valid json");
        assert_eq!(
            payload.get("version").and_then(Value::as_str),
            Some(CONTRACT_VERSION)
        );
    }

    #[test]
    fn search_title_cleanup_removes_engine_breadcrumbs() {
        assert_eq!(
            clean_search_result_title(
                "IANA iana.org › help › example-domains Example Domains",
                "www.iana.org"
            ),
            "Example Domains"
        );
        assert_eq!(
            clean_search_result_title("iana.org Example Domains", "iana.org"),
            "Example Domains"
        );
    }

    #[test]
    fn preferred_search_engine_loads_legacy_unversioned_metadata() {
        let data_root = temporary_directory("search-preferred-engine-legacy");
        let metadata_file = data_root.join("browser-search/preferred-engine.json");
        fs::create_dir_all(
            metadata_file
                .parent()
                .expect("metadata parent directory should exist"),
        )
        .expect("metadata parent directory should be created");
        fs::write(&metadata_file, "{\n  \"engine\": \"google\"\n}")
            .expect("legacy metadata should be written");

        assert_eq!(
            load_preferred_search_engine_from(&metadata_file)
                .expect("legacy preferred engine should load"),
            Some(SearchEngine::Google)
        );
    }

    #[test]
    fn default_search_profile_dir_lives_under_data_root() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let data_root = temporary_directory("search-profile-root");
        let previous = std::env::var_os("TOUCH_BROWSER_DATA_ROOT");
        std::env::set_var("TOUCH_BROWSER_DATA_ROOT", &data_root);

        assert_eq!(
            default_search_profile_dir(SearchEngine::Google),
            data_root
                .canonicalize()
                .unwrap_or(data_root.clone())
                .join("browser-search/profiles/google-default")
        );

        restore_env("TOUCH_BROWSER_DATA_ROOT", previous);
    }

    #[test]
    fn search_profile_state_tracks_challenge_then_manual_recovery() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let data_root = temporary_directory("search-profile-state");
        let previous = std::env::var_os("TOUCH_BROWSER_DATA_ROOT");
        std::env::set_var("TOUCH_BROWSER_DATA_ROOT", &data_root);
        let profile_dir = data_root.join("browser-search/profiles/google-default");
        let metadata_file = data_root.join("browser-search/google.profile-state.json");

        let challenged = record_search_profile_result(
            SearchEngine::Google,
            &profile_dir,
            SearchReportStatus::Challenge,
            true,
            "2026-04-11T01:00:00+09:00",
        )
        .expect("challenge state should save");
        assert_eq!(challenged.consecutive_challenges, 1);
        assert_eq!(
            challenged.last_challenge_at.as_deref(),
            Some("2026-04-11T01:00:00+09:00")
        );

        let recovered = record_search_profile_result(
            SearchEngine::Google,
            &profile_dir,
            SearchReportStatus::Ready,
            true,
            "2026-04-11T01:05:00+09:00",
        )
        .expect("recovery state should save");
        assert_eq!(recovered.consecutive_challenges, 0);
        assert_eq!(
            recovered.last_success_at.as_deref(),
            Some("2026-04-11T01:05:00+09:00")
        );
        assert_eq!(
            recovered.last_manual_recovery_at.as_deref(),
            Some("2026-04-11T01:05:00+09:00")
        );
        assert_eq!(
            load_search_profile_state_from(&metadata_file)
                .expect("profile state should reload")
                .expect("profile state should exist"),
            recovered
        );
        assert_eq!(recovered.version, CONTRACT_VERSION);

        restore_env("TOUCH_BROWSER_DATA_ROOT", previous);
    }

    #[test]
    fn canonicalize_search_result_url_normalizes_mdn_locale_pages_to_en_us() {
        let url = Url::parse(
            "https://developer.mozilla.org/ko/docs/Web/API/AbortController?utm_source=test",
        )
        .expect("url should parse");

        assert_eq!(
            canonicalize_search_result_url(url),
            Some("https://developer.mozilla.org/en-US/docs/Web/API/AbortController".to_string())
        );
    }

    #[test]
    fn query_authority_bonus_prefers_nodejs_docs_over_secondary_official_hosts() {
        let node_result = SearchResultItem {
            rank: 3,
            title: "Node.js URL API".to_string(),
            url: "https://nodejs.org/api/url.html".to_string(),
            domain: "nodejs.org".to_string(),
            snippet: Some("The WHATWG URL API in Node.js.".to_string()),
            stable_ref: None,
            official_likely: true,
            selection_score: None,
            recommended_surface: None,
        };
        let whatwg_result = SearchResultItem {
            rank: 1,
            title: "URL Standard".to_string(),
            url: "https://url.spec.whatwg.org/".to_string(),
            domain: "url.spec.whatwg.org".to_string(),
            snippet: Some("The living WHATWG URL specification.".to_string()),
            stable_ref: None,
            official_likely: true,
            selection_score: None,
            recommended_surface: None,
        };
        let query = "WHATWG URL API Node.js docs official";

        assert!(
            authority_domain_bonus(query, &node_result)
                > authority_domain_bonus(query, &whatwg_result),
            "Node.js authority hints should outrank the broader WHATWG spec for a Node.js docs query"
        );
        assert!(
            selection_score_for_result(query, &node_result)
                > selection_score_for_result(query, &whatwg_result),
            "selection score should open the Node.js doc ahead of the broader spec page"
        );
    }

    #[test]
    fn query_authority_bonus_prefers_primary_spec_when_query_targets_whatwg() {
        let node_result = SearchResultItem {
            rank: 1,
            title: "Node.js URL API".to_string(),
            url: "https://nodejs.org/api/url.html".to_string(),
            domain: "nodejs.org".to_string(),
            snippet: Some("The WHATWG URL API in Node.js.".to_string()),
            stable_ref: None,
            official_likely: true,
            selection_score: None,
            recommended_surface: None,
        };
        let whatwg_result = SearchResultItem {
            rank: 2,
            title: "URL Standard".to_string(),
            url: "https://url.spec.whatwg.org/".to_string(),
            domain: "url.spec.whatwg.org".to_string(),
            snippet: Some("The living WHATWG URL specification.".to_string()),
            stable_ref: None,
            official_likely: true,
            selection_score: None,
            recommended_surface: None,
        };
        let query = "WHATWG URL standard official";

        assert!(
            authority_domain_bonus(query, &whatwg_result)
                > authority_domain_bonus(query, &node_result),
            "The WHATWG spec should be the primary authority when the query targets the standard"
        );
        assert!(
            selection_score_for_result(query, &whatwg_result)
                > selection_score_for_result(query, &node_result),
            "selection score should keep the WHATWG spec ahead when the query explicitly targets it"
        );
    }

    #[test]
    fn browser_research_identity_applies_to_remote_docs_hosts() {
        assert!(should_use_browser_research_identity(
            "https://developer.mozilla.org/ko/docs/Web/API/AbortController"
        ));
        assert!(should_use_browser_research_identity(
            "https://www.google.com/search?q=abortcontroller"
        ));
    }

    #[test]
    fn browser_research_identity_skips_local_and_non_http_targets() {
        assert!(!should_use_browser_research_identity(
            "http://127.0.0.1:4173/docs-shell"
        ));
        assert!(!should_use_browser_research_identity(
            "http://localhost:4173/spa"
        ));
        assert!(!should_use_browser_research_identity(
            "fixture://docs-shell"
        ));
    }

    #[test]
    fn search_profile_state_loads_legacy_unversioned_metadata() {
        let _guard = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let data_root = temporary_directory("search-profile-state-legacy");
        let previous = std::env::var_os("TOUCH_BROWSER_DATA_ROOT");
        std::env::set_var("TOUCH_BROWSER_DATA_ROOT", &data_root);
        let metadata_file = data_root.join("browser-search/google.profile-state.json");
        fs::create_dir_all(
            metadata_file
                .parent()
                .expect("metadata parent directory should exist"),
        )
        .expect("metadata parent directory should be created");
        fs::write(
            &metadata_file,
            "{\n  \"engine\": \"google\",\n  \"profile_dir\": \"/tmp/google-default\",\n  \"last_success_at\": null,\n  \"last_challenge_at\": \"2026-04-11T01:00:00+09:00\",\n  \"last_manual_recovery_at\": null,\n  \"consecutive_challenges\": 1\n}",
        )
        .expect("legacy profile state should be written");

        let loaded = load_search_profile_state_from(&metadata_file)
            .expect("legacy profile state should load")
            .expect("legacy profile state should exist");
        assert_eq!(loaded.version, CONTRACT_VERSION);
        assert_eq!(loaded.engine, SearchEngine::Google);
        assert_eq!(loaded.profile_dir, "/tmp/google-default");
        assert_eq!(loaded.consecutive_challenges, 1);

        restore_env("TOUCH_BROWSER_DATA_ROOT", previous);
    }

    #[test]
    fn infers_latest_search_engine_from_latest_session_file_when_metadata_is_missing() {
        let data_root = temporary_directory("search-latest-engine");
        let search_output_dir = data_root.join("browser-search");
        fs::create_dir_all(&search_output_dir).expect("search output dir should exist");

        let google_session = search_output_dir.join("google.search-session.json");
        let brave_session = search_output_dir.join("brave.search-session.json");
        fs::write(&google_session, "{}\n").expect("google session should exist");
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&brave_session, "{}\n").expect("brave session should exist");

        let latest = latest_search_session_file_in(&search_output_dir)
            .expect("latest session should resolve")
            .expect("latest session should exist");
        assert_eq!(
            infer_search_engine_from_session_file(&latest),
            Some(SearchEngine::Brave)
        );
    }

    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }

    fn env_lock() -> &'static Mutex<()> {
        crate::test_env_lock()
    }
}
