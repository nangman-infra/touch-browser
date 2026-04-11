use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::browser_models::*;
use crate::application::browser_session::BrowserLoadDiagnostics;
use crate::application::{
    browser_session::{
        BrowserActionSource, BrowserActionTraceEntry, BrowserCliSession, BrowserOrigin,
        BrowserSessionContext, ObservedBrowserDocument, PersistedBrowserState,
    },
    search_support::is_search_results_target,
};
use crate::infrastructure::fixtures::load_fixture_catalog;
use crate::interface::{
    cli_error::CliError,
    cli_models::SecretPrefill,
    cli_support::{
        current_timestamp, is_fixture_target, node_executable, repo_root, resource_root,
    },
};
use touch_browser_contracts::{
    PolicyProfile, SearchReport, SessionMode, SnapshotBlock, SnapshotDocument, SourceRisk,
    SourceType, CONTRACT_VERSION,
};
use touch_browser_observation::{
    recommend_requested_tokens, ObservationCompiler, ObservationInput,
};
use touch_browser_runtime::{ReadOnlyRuntime, ReadOnlySession, RuntimeError};

#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: Value,
    method: &'static str,
    params: T,
}

#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Value,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn open_browser_session(
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
    let runtime = ReadOnlyRuntime::default();
    let mut session = runtime.start_session(session_id, timestamp);
    let observed = browser_document(
        target,
        requested_budget,
        source_risk,
        source_label,
        headed,
        browser_context_dir.clone(),
        browser_profile_dir.clone(),
    )?;
    let snapshot = runtime.open_snapshot(
        &mut session,
        target,
        observed.snapshot,
        observed.source_risk.clone(),
        observed.source_label.clone(),
        timestamp,
    )?;

    Ok(BrowserSessionContext {
        runtime,
        session,
        snapshot,
        source_risk: observed.source_risk,
        source_label: observed.source_label,
        browser_state: observed.browser_state,
        load_diagnostics: observed.load_diagnostics,
        browser_context_dir: observed.browser_context_dir,
        browser_profile_dir: observed.browser_profile_dir,
    })
}

pub(crate) fn browser_document(
    target: &str,
    requested_budget: usize,
    source_risk: Option<SourceRisk>,
    source_label: Option<String>,
    headed: bool,
    browser_context_dir: Option<String>,
    browser_profile_dir: Option<String>,
) -> Result<ObservedBrowserDocument, CliError> {
    if is_fixture_target(target) {
        let catalog = load_fixture_catalog()?;
        let document = catalog
            .get(target)
            .ok_or_else(|| RuntimeError::UnknownSource(target.to_string()))?;
        let effective_budget = recommend_requested_tokens(&document.html, requested_budget);
        let capture = invoke_playwright_snapshot(PlaywrightSnapshotParams {
            url: None,
            html: Some(document.html.clone()),
            context_dir: browser_context_dir.clone(),
            profile_dir: browser_profile_dir.clone(),
            budget: effective_budget,
            headless: !headed,
            search_identity: false,
            manual_recovery: false,
        })?;
        let snapshot = compile_browser_snapshot(target, &capture.html, effective_budget)?;

        return Ok(ObservedBrowserDocument {
            snapshot,
            source_risk: source_risk.unwrap_or(document.source_risk.clone()),
            source_label: source_label.or_else(|| document.source_label.clone()),
            browser_state: PersistedBrowserState {
                current_url: capture.final_url,
                current_html: capture.html,
            },
            load_diagnostics: BrowserLoadDiagnostics {
                wait_strategy: capture.diagnostics.wait_strategy,
                wait_budget_ms: capture.diagnostics.wait_budget_ms,
                wait_consumed_ms: capture.diagnostics.wait_consumed_ms,
                wait_stop_reason: capture.diagnostics.wait_stop_reason,
            },
            browser_context_dir,
            browser_profile_dir,
        });
    }

    let capture = invoke_playwright_snapshot(PlaywrightSnapshotParams {
        url: Some(target.to_string()),
        html: None,
        context_dir: browser_context_dir.clone(),
        profile_dir: browser_profile_dir.clone(),
        budget: requested_budget,
        headless: !headed,
        search_identity: is_search_results_target(target),
        manual_recovery: headed && is_search_results_target(target),
    })?;
    let effective_budget = recommend_requested_tokens(&capture.html, requested_budget);
    let snapshot = compile_browser_snapshot(&capture.final_url, &capture.html, effective_budget)?;

    Ok(ObservedBrowserDocument {
        snapshot,
        source_risk: source_risk.unwrap_or(SourceRisk::Low),
        source_label,
        browser_state: PersistedBrowserState {
            current_url: capture.final_url,
            current_html: capture.html,
        },
        load_diagnostics: BrowserLoadDiagnostics {
            wait_strategy: capture.diagnostics.wait_strategy,
            wait_budget_ms: capture.diagnostics.wait_budget_ms,
            wait_consumed_ms: capture.diagnostics.wait_consumed_ms,
            wait_stop_reason: capture.diagnostics.wait_stop_reason,
        },
        browser_context_dir,
        browser_profile_dir,
    })
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_browser_cli_session(
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
    BrowserCliSession {
        version: CONTRACT_VERSION.to_string(),
        headless,
        requested_budget,
        session: session.clone(),
        browser_state,
        browser_context_dir,
        browser_profile_dir,
        browser_origin,
        allowlisted_domains,
        browser_trace,
        approved_risks: BTreeSet::new(),
        latest_search,
    }
}

pub(crate) fn save_browser_cli_session(
    path: &Path,
    persisted: &BrowserCliSession,
) -> Result<(), CliError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, serde_json::to_vec_pretty(&persisted)?).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

pub(crate) fn load_browser_cli_session(path: &Path) -> Result<BrowserCliSession, CliError> {
    let raw = fs::read_to_string(path).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| CliError::JsonPath {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn browser_secret_store_path(path: &Path) -> PathBuf {
    let mut secret_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("touch-browser-session")
        .to_string();
    secret_name.push_str(".secrets.json");

    path.parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .join(secret_name)
}

pub(crate) fn load_browser_cli_secrets(path: &Path) -> Result<BTreeMap<String, String>, CliError> {
    if !path.exists() {
        return Ok(BTreeMap::new());
    }

    let raw = fs::read_to_string(path).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    serde_json::from_str(&raw).map_err(|source| CliError::JsonPath {
        path: path.display().to_string(),
        source,
    })
}

pub(crate) fn save_browser_cli_secrets(
    path: &Path,
    secrets: &BTreeMap<String, String>,
) -> Result<(), CliError> {
    if secrets.is_empty() {
        if path.exists() {
            fs::remove_file(path)?;
        }
        return Ok(());
    }

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::write(path, serde_json::to_vec_pretty(secrets)?).map_err(|source| CliError::IoPath {
        path: path.display().to_string(),
        source,
    })?;
    Ok(())
}

pub(crate) fn browser_context_dir_for_session_file(path: &Path) -> PathBuf {
    let mut context_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "touch-browser-session".to_string());
    context_name.push_str(".browser-context");

    path.parent()
        .unwrap_or_else(|| Path::new("/tmp"))
        .join(context_name)
}

pub(crate) fn current_browser_action_source(
    persisted: &BrowserCliSession,
) -> Result<BrowserActionSource, CliError> {
    let current_record = persisted
        .session
        .current_snapshot_record()
        .ok_or(RuntimeError::NoCurrentSnapshot)?;
    let source_url = persisted
        .session
        .state
        .current_url
        .clone()
        .ok_or(RuntimeError::MissingCurrentUrl)?;

    if let Some(browser_state) = persisted.browser_state.as_ref() {
        let use_live_url = (persisted.browser_context_dir.is_some()
            || persisted.browser_profile_dir.is_some())
            && !is_fixture_target(&source_url)
            && !browser_state.current_url.starts_with("about:blank");
        return Ok(BrowserActionSource {
            source_url,
            url: Some(browser_state.current_url.clone()),
            html: if use_live_url {
                None
            } else {
                Some(browser_state.current_html.clone())
            },
            context_dir: persisted.browser_context_dir.clone(),
            profile_dir: persisted.browser_profile_dir.clone(),
            source_risk: current_record.source_risk.clone(),
            source_label: current_record.source_label.clone(),
        });
    }

    if is_fixture_target(&source_url) {
        let catalog = load_fixture_catalog()?;
        let document = catalog
            .get(&source_url)
            .ok_or_else(|| RuntimeError::UnknownSource(source_url.clone()))?;
        return Ok(BrowserActionSource {
            source_url,
            url: None,
            html: Some(document.html.clone()),
            context_dir: persisted.browser_context_dir.clone(),
            profile_dir: persisted.browser_profile_dir.clone(),
            source_risk: current_record.source_risk.clone(),
            source_label: current_record.source_label.clone(),
        });
    }

    Ok(BrowserActionSource {
        source_url: source_url.clone(),
        url: Some(source_url),
        html: None,
        context_dir: persisted.browser_context_dir.clone(),
        profile_dir: persisted.browser_profile_dir.clone(),
        source_risk: current_record.source_risk.clone(),
        source_label: current_record.source_label.clone(),
    })
}

pub(crate) fn resolved_browser_source_url(source: &BrowserActionSource, final_url: &str) -> String {
    if source.html.is_some() {
        return source.source_url.clone();
    }

    if source.url.is_some() {
        return final_url.to_string();
    }

    source.source_url.clone()
}

pub(crate) fn current_snapshot_ref_text(
    session: &ReadOnlySession,
    target_ref: &str,
) -> Result<String, CliError> {
    let text = resolve_session_block(session, target_ref)
        .map(|block| block.text.clone())
        .ok_or_else(|| RuntimeError::MissingHref(target_ref.to_string()))?;
    Ok(text)
}

pub(crate) fn current_snapshot_ref_href(
    session: &ReadOnlySession,
    target_ref: &str,
) -> Option<String> {
    resolve_session_block(session, target_ref)
        .and_then(|block| block.attributes.get("href"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn current_snapshot_ref_tag_name(
    session: &ReadOnlySession,
    target_ref: &str,
) -> Option<String> {
    resolve_session_block(session, target_ref)
        .and_then(|block| block.attributes.get("tagName"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn current_snapshot_ref_dom_path_hint(
    session: &ReadOnlySession,
    target_ref: &str,
) -> Option<String> {
    resolve_session_block(session, target_ref)
        .and_then(|block| block.evidence.dom_path_hint.clone())
}

pub(crate) fn current_snapshot_ref_name(
    session: &ReadOnlySession,
    target_ref: &str,
) -> Option<String> {
    resolve_session_block(session, target_ref)
        .and_then(|block| block.attributes.get("name"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn current_snapshot_ref_input_type(
    session: &ReadOnlySession,
    target_ref: &str,
) -> Option<String> {
    resolve_session_block(session, target_ref)
        .and_then(|block| block.attributes.get("inputType"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn current_snapshot_ref_is_sensitive(
    session: &ReadOnlySession,
    target_ref: &str,
) -> bool {
    let Some(block) = resolve_session_block(session, target_ref) else {
        return false;
    };

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
        || name.contains("otp")
        || name.contains("token")
        || name.contains("code")
        || text.contains("password")
        || text.contains("otp")
        || text.contains("verification")
}

pub(crate) fn resolve_session_block<'a>(
    session: &'a ReadOnlySession,
    target_ref: &str,
) -> Option<&'a SnapshotBlock> {
    session.snapshots.iter().rev().find_map(|record| {
        record
            .snapshot
            .blocks
            .iter()
            .find(|block| block.stable_ref == target_ref)
    })
}

pub(crate) fn collect_submit_prefill(
    persisted: &BrowserCliSession,
    extra_prefill: &[SecretPrefill],
) -> Vec<PlaywrightTypePrefill> {
    let mut prefills = Vec::new();

    for entry in &persisted.browser_trace {
        if entry.action != "type" || entry.redacted {
            continue;
        }

        let Some(target_ref) = entry.target_ref.as_ref() else {
            continue;
        };
        let Some(value) = entry.text_value.as_ref() else {
            continue;
        };

        let prefill = PlaywrightTypePrefill {
            target_ref: target_ref.clone(),
            target_text: current_snapshot_ref_text(&persisted.session, target_ref).ok(),
            target_tag_name: current_snapshot_ref_tag_name(&persisted.session, target_ref),
            target_dom_path_hint: current_snapshot_ref_dom_path_hint(
                &persisted.session,
                target_ref,
            ),
            target_ordinal_hint: stable_ref_ordinal_hint(target_ref),
            target_name: current_snapshot_ref_name(&persisted.session, target_ref),
            target_input_type: current_snapshot_ref_input_type(&persisted.session, target_ref),
            value: value.clone(),
        };

        if let Some(index) = prefills
            .iter()
            .position(|existing: &PlaywrightTypePrefill| existing.target_ref == prefill.target_ref)
        {
            prefills.remove(index);
        }
        prefills.push(prefill);
    }

    for entry in extra_prefill {
        let prefill = PlaywrightTypePrefill {
            target_ref: entry.target_ref.clone(),
            target_text: current_snapshot_ref_text(&persisted.session, &entry.target_ref).ok(),
            target_tag_name: current_snapshot_ref_tag_name(&persisted.session, &entry.target_ref),
            target_dom_path_hint: current_snapshot_ref_dom_path_hint(
                &persisted.session,
                &entry.target_ref,
            ),
            target_ordinal_hint: stable_ref_ordinal_hint(&entry.target_ref),
            target_name: current_snapshot_ref_name(&persisted.session, &entry.target_ref),
            target_input_type: current_snapshot_ref_input_type(
                &persisted.session,
                &entry.target_ref,
            ),
            value: entry.value.clone(),
        };

        if let Some(index) = prefills
            .iter()
            .position(|existing: &PlaywrightTypePrefill| existing.target_ref == prefill.target_ref)
        {
            prefills.remove(index);
        }
        prefills.push(prefill);
    }

    prefills
}

pub(crate) fn mark_browser_session_interactive(persisted: &mut BrowserCliSession) {
    persisted.session.state.mode = SessionMode::Interactive;
    if matches!(
        persisted.session.state.policy_profile,
        PolicyProfile::ResearchReadOnly | PolicyProfile::ResearchRestricted
    ) {
        persisted.session.state.policy_profile = PolicyProfile::InteractiveReview;
    }
    persisted.session.state.status = touch_browser_contracts::SessionStatus::Active;
}

pub(crate) fn next_session_timestamp(session: &ReadOnlySession) -> String {
    let _ = session;
    current_timestamp()
}

pub(crate) fn stable_ref_ordinal_hint(target_ref: &str) -> Option<usize> {
    target_ref
        .rsplit(':')
        .next()
        .and_then(|segment| segment.parse::<usize>().ok())
        .filter(|ordinal| *ordinal > 1)
}

pub(crate) fn compile_browser_snapshot(
    source_url: &str,
    html: &str,
    requested_tokens: usize,
) -> Result<SnapshotDocument, CliError> {
    ObservationCompiler
        .compile(&ObservationInput::new(
            source_url.to_string(),
            SourceType::Playwright,
            html.to_string(),
            requested_tokens,
        ))
        .map_err(CliError::Observation)
}

pub(crate) fn invoke_playwright_snapshot(
    params: PlaywrightSnapshotParams,
) -> Result<PlaywrightSnapshotResult, CliError> {
    invoke_playwright_request("browser.snapshot", json!("cli-browser-snapshot"), params)
}

pub(crate) fn invoke_playwright_follow(
    params: PlaywrightFollowParams,
) -> Result<PlaywrightFollowResult, CliError> {
    invoke_playwright_request("browser.follow", json!("cli-browser-follow"), params)
}

pub(crate) fn invoke_playwright_click(
    params: PlaywrightClickParams,
) -> Result<PlaywrightClickResult, CliError> {
    invoke_playwright_request("browser.click", json!("cli-browser-click"), params)
}

pub(crate) fn invoke_playwright_type(
    params: PlaywrightTypeParams,
) -> Result<PlaywrightTypeResult, CliError> {
    invoke_playwright_request("browser.type", json!("cli-browser-type"), params)
}

pub(crate) fn invoke_playwright_submit(
    params: PlaywrightSubmitParams,
) -> Result<PlaywrightSubmitResult, CliError> {
    invoke_playwright_request("browser.submit", json!("cli-browser-submit"), params)
}

pub(crate) fn invoke_playwright_paginate(
    params: PlaywrightPaginateParams,
) -> Result<PlaywrightPaginateResult, CliError> {
    invoke_playwright_request("browser.paginate", json!("cli-browser-paginate"), params)
}

pub(crate) fn invoke_playwright_expand(
    params: PlaywrightExpandParams,
) -> Result<PlaywrightExpandResult, CliError> {
    invoke_playwright_request("browser.expand", json!("cli-browser-expand"), params)
}

fn invoke_playwright_request<Params, ResultType>(
    method: &'static str,
    id: Value,
    params: Params,
) -> Result<ResultType, CliError>
where
    Params: Serialize,
    ResultType: for<'de> Deserialize<'de>,
{
    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        id,
        method,
        params,
    };
    let request_body = serde_json::to_vec(&request)?;
    let repo_root = repo_root();
    let runtime_root = resource_root();
    let mut child = build_playwright_adapter_command(&repo_root, &runtime_root)?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    {
        let stdin = child.stdin.as_mut().ok_or_else(|| {
            CliError::Adapter("Failed to open Playwright adapter stdin.".to_string())
        })?;
        stdin.write_all(&request_body)?;
    }
    let _ = child.stdin.take();

    let output = child.wait_with_output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let detail = if !stderr.is_empty() { stderr } else { stdout };
        return Err(CliError::Adapter(format!(
            "Playwright adapter failed with status {}: {detail}",
            output.status
        )));
    }

    let response: JsonRpcResponse<ResultType> = serde_json::from_slice(&output.stdout)?;
    match (response.result, response.error) {
        (Some(result), None) => Ok(result),
        (None, Some(error)) => Err(CliError::Adapter(format!(
            "Playwright adapter returned JSON-RPC error {}: {}",
            error.code, error.message
        ))),
        _ => Err(CliError::Adapter(
            "Playwright adapter returned an invalid JSON-RPC envelope.".to_string(),
        )),
    }
}

fn build_playwright_adapter_command(
    repo_root: &Path,
    runtime_root: &Path,
) -> Result<Command, CliError> {
    if let Some(explicit_command) = std::env::var_os("TOUCH_BROWSER_PLAYWRIGHT_ADAPTER_COMMAND")
        .filter(|value| !value.is_empty())
    {
        let mut command = Command::new("sh");
        command
            .args(["-lc", explicit_command.to_string_lossy().as_ref()])
            .current_dir(repo_root);
        return Ok(command);
    }

    if let Some(packaged_entry) = resolve_packaged_playwright_adapter(runtime_root) {
        let mut command = Command::new(node_executable());
        command.arg(packaged_entry).current_dir(runtime_root);
        return Ok(command);
    }

    if let Some(compiled_entry) = resolve_compiled_playwright_adapter(repo_root) {
        let mut command = Command::new(node_executable());
        command.arg(compiled_entry).current_dir(repo_root);
        return Ok(command);
    }

    let source_entry = repo_root.join("adapters/playwright/src/index.ts");
    if source_entry.is_file() {
        let mut command = Command::new("pnpm");
        command
            .args(["exec", "tsx", "adapters/playwright/src/index.ts"])
            .current_dir(repo_root);
        return Ok(command);
    }

    Err(CliError::Adapter(
        "Could not resolve a Playwright adapter entrypoint for this installation.".to_string(),
    ))
}

fn resolve_packaged_playwright_adapter(runtime_root: &Path) -> Option<PathBuf> {
    [
        runtime_root.join("adapters/playwright/index.js"),
        runtime_root.join("adapters/playwright/dist/index.js"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn resolve_compiled_playwright_adapter(repo_root: &Path) -> Option<PathBuf> {
    [
        repo_root.join("adapters/playwright/dist-runtime/src/index.js"),
        repo_root.join("adapters/playwright/dist/src/index.js"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    use crate::interface::cli_error::CliError;

    use super::{
        build_playwright_adapter_command, resolve_compiled_playwright_adapter,
        resolve_packaged_playwright_adapter,
    };

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
    fn resolves_packaged_playwright_adapter_from_runtime_root() {
        let runtime_root = temporary_directory("runtime-root");
        let packaged_entry = runtime_root.join("adapters/playwright/index.js");
        fs::create_dir_all(
            packaged_entry
                .parent()
                .expect("packaged adapter parent should exist"),
        )
        .expect("packaged adapter parent should be created");
        fs::write(&packaged_entry, "export {};\n").expect("packaged adapter should exist");

        assert_eq!(
            resolve_packaged_playwright_adapter(&runtime_root),
            Some(packaged_entry)
        );
    }

    #[test]
    fn resolves_compiled_playwright_adapter_from_repo_root() {
        let repo_root = temporary_directory("compiled-adapter");
        let compiled_entry = repo_root.join("adapters/playwright/dist-runtime/src/index.js");
        fs::create_dir_all(
            compiled_entry
                .parent()
                .expect("compiled adapter parent should exist"),
        )
        .expect("compiled adapter parent should be created");
        fs::write(&compiled_entry, "export {};\n").expect("compiled adapter should exist");

        assert_eq!(
            resolve_compiled_playwright_adapter(&repo_root),
            Some(compiled_entry)
        );
    }

    #[test]
    fn adapter_command_errors_when_no_entrypoint_exists() {
        let repo_root = temporary_directory("adapter-repo");
        let runtime_root = temporary_directory("adapter-runtime");

        let error = build_playwright_adapter_command(&repo_root, &runtime_root)
            .expect_err("missing adapter should be rejected");
        assert!(matches!(error, CliError::Adapter(_)));
    }
}
