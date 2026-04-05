use serde::Serialize;
use serde_json::Value;

use crate::{CliError, OutputFormat, SearchResultItem, SessionSynthesisReport, CONTRACT_VERSION};

fn to_value<T: Serialize>(value: T) -> Result<Value, CliError> {
    Ok(serde_json::to_value(value)?)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcResultEnvelope {
    jsonrpc: &'static str,
    id: Value,
    result: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcErrorDetail {
    code: i64,
    message: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonRpcErrorEnvelope {
    jsonrpc: &'static str,
    id: Value,
    error: JsonRpcErrorDetail,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatusResponse {
    status: &'static str,
    transport: &'static str,
    version: &'static str,
    daemon: bool,
    methods: &'static [&'static str],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionTabResultResponse {
    session_id: String,
    tab_id: String,
    result: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionCreatedResponse {
    session_id: String,
    active_tab_id: String,
    headless: bool,
    allow_domains: Vec<String>,
    tab_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchOpenResultResponse {
    session_id: String,
    search_tab_id: String,
    opened_tab_id: String,
    selection_strategy: String,
    selected_result: SearchResultItem,
    result: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchOpenedTabResponse {
    tab_id: String,
    selected_result: SearchResultItem,
    result: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SearchOpenTopResponse {
    session_id: String,
    search_tab_id: String,
    opened_count: usize,
    opened_tabs: Vec<SearchOpenedTabResponse>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionApprovedResponse {
    session_id: String,
    approved_risks: Vec<String>,
    policy_profile: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretStoreResponse {
    session_id: String,
    stored: bool,
    target_ref: String,
    secret_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SecretClearResponse {
    session_id: String,
    removed: bool,
    secret_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionSynthesizeTabReport {
    tab_id: String,
    report: SessionSynthesisReport,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionSynthesizeResponse {
    session_id: String,
    active_tab_id: Option<String>,
    tab_count: usize,
    format: OutputFormat,
    markdown: Option<String>,
    report: SessionSynthesisReport,
    tab_reports: Vec<SessionSynthesizeTabReport>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TabOpenResponse {
    session_id: String,
    active_tab_id: String,
    tab: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TabListResponse {
    session_id: String,
    active_tab_id: Option<String>,
    tabs: Vec<Value>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TabSelectResponse {
    session_id: String,
    active_tab_id: String,
    tab: Value,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TabSummaryResponse {
    tab_id: String,
    active: bool,
    session_file: String,
    has_state: bool,
    current_url: Option<String>,
    visited_url_count: usize,
    snapshot_count: usize,
    latest_search_query: Option<String>,
    latest_search_result_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TabCloseResponse {
    session_id: String,
    tab_id: String,
    removed: bool,
    removed_state: bool,
    active_tab_id: Option<String>,
    remaining_tab_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionCloseResponse {
    session_id: String,
    removed: bool,
    removed_tabs: usize,
}

pub(crate) fn present_json_rpc_result(id: Value, result: Value) -> Value {
    serde_json::to_value(JsonRpcResultEnvelope {
        jsonrpc: "2.0",
        id,
        result,
    })
    .expect("json-rpc result should serialize")
}

pub(crate) fn present_json_rpc_error(id: Value, code: i64, message: String) -> Value {
    serde_json::to_value(JsonRpcErrorEnvelope {
        jsonrpc: "2.0",
        id,
        error: JsonRpcErrorDetail { code, message },
    })
    .expect("json-rpc error should serialize")
}

pub(crate) fn present_runtime_status(methods: &'static [&'static str]) -> Result<Value, CliError> {
    to_value(RuntimeStatusResponse {
        status: "ready",
        transport: "stdio-json-rpc",
        version: CONTRACT_VERSION,
        daemon: true,
        methods,
    })
}

pub(crate) fn present_session_tab_result(
    session_id: impl Into<String>,
    tab_id: impl Into<String>,
    result: Value,
) -> Result<Value, CliError> {
    to_value(SessionTabResultResponse {
        session_id: session_id.into(),
        tab_id: tab_id.into(),
        result,
    })
}

pub(crate) fn present_session_created(
    session_id: impl Into<String>,
    active_tab_id: impl Into<String>,
    headless: bool,
    allow_domains: Vec<String>,
    tab_count: usize,
) -> Result<Value, CliError> {
    to_value(SessionCreatedResponse {
        session_id: session_id.into(),
        active_tab_id: active_tab_id.into(),
        headless,
        allow_domains,
        tab_count,
    })
}

pub(crate) fn present_search_open_result(
    session_id: impl Into<String>,
    search_tab_id: impl Into<String>,
    opened_tab_id: impl Into<String>,
    selection_strategy: impl Into<String>,
    selected_result: SearchResultItem,
    result: Value,
) -> Result<Value, CliError> {
    to_value(SearchOpenResultResponse {
        session_id: session_id.into(),
        search_tab_id: search_tab_id.into(),
        opened_tab_id: opened_tab_id.into(),
        selection_strategy: selection_strategy.into(),
        selected_result,
        result,
    })
}

pub(crate) fn present_search_open_top(
    session_id: impl Into<String>,
    search_tab_id: impl Into<String>,
    opened_tabs: Vec<(String, SearchResultItem, Value)>,
) -> Result<Value, CliError> {
    let opened_tabs = opened_tabs
        .into_iter()
        .map(
            |(tab_id, selected_result, result)| SearchOpenedTabResponse {
                tab_id,
                selected_result,
                result,
            },
        )
        .collect::<Vec<_>>();
    to_value(SearchOpenTopResponse {
        session_id: session_id.into(),
        search_tab_id: search_tab_id.into(),
        opened_count: opened_tabs.len(),
        opened_tabs,
    })
}

pub(crate) fn present_session_approved(
    session_id: impl Into<String>,
    approved_risks: Vec<String>,
    policy_profile: impl Into<String>,
) -> Result<Value, CliError> {
    to_value(SessionApprovedResponse {
        session_id: session_id.into(),
        approved_risks,
        policy_profile: policy_profile.into(),
    })
}

pub(crate) fn present_secret_store(
    session_id: impl Into<String>,
    target_ref: impl Into<String>,
    secret_count: usize,
) -> Result<Value, CliError> {
    to_value(SecretStoreResponse {
        session_id: session_id.into(),
        stored: true,
        target_ref: target_ref.into(),
        secret_count,
    })
}

pub(crate) fn present_secret_clear(
    session_id: impl Into<String>,
    removed: bool,
    secret_count: usize,
) -> Result<Value, CliError> {
    to_value(SecretClearResponse {
        session_id: session_id.into(),
        removed,
        secret_count,
    })
}

pub(crate) fn present_session_synthesize(
    session_id: impl Into<String>,
    active_tab_id: Option<String>,
    tab_count: usize,
    format: OutputFormat,
    markdown: Option<String>,
    report: SessionSynthesisReport,
    tab_reports: Vec<(String, SessionSynthesisReport)>,
) -> Result<Value, CliError> {
    let tab_reports = tab_reports
        .into_iter()
        .map(|(tab_id, report)| SessionSynthesizeTabReport { tab_id, report })
        .collect::<Vec<_>>();
    to_value(SessionSynthesizeResponse {
        session_id: session_id.into(),
        active_tab_id,
        tab_count,
        format,
        markdown,
        report,
        tab_reports,
    })
}

pub(crate) fn present_tab_open(
    session_id: impl Into<String>,
    active_tab_id: impl Into<String>,
    tab: Value,
) -> Result<Value, CliError> {
    to_value(TabOpenResponse {
        session_id: session_id.into(),
        active_tab_id: active_tab_id.into(),
        tab,
    })
}

pub(crate) fn present_tab_list(
    session_id: impl Into<String>,
    active_tab_id: Option<String>,
    tabs: Vec<Value>,
) -> Result<Value, CliError> {
    to_value(TabListResponse {
        session_id: session_id.into(),
        active_tab_id,
        tabs,
    })
}

pub(crate) fn present_tab_select(
    session_id: impl Into<String>,
    active_tab_id: impl Into<String>,
    tab: Value,
) -> Result<Value, CliError> {
    to_value(TabSelectResponse {
        session_id: session_id.into(),
        active_tab_id: active_tab_id.into(),
        tab,
    })
}

pub(crate) fn present_tab_summary(
    tab_id: impl Into<String>,
    active: bool,
    session_file: impl Into<String>,
    has_state: bool,
    current_url: Option<String>,
    visited_url_count: usize,
    snapshot_count: usize,
    latest_search_query: Option<String>,
    latest_search_result_count: usize,
) -> Result<Value, CliError> {
    to_value(TabSummaryResponse {
        tab_id: tab_id.into(),
        active,
        session_file: session_file.into(),
        has_state,
        current_url,
        visited_url_count,
        snapshot_count,
        latest_search_query,
        latest_search_result_count,
    })
}

pub(crate) fn present_tab_close(
    session_id: impl Into<String>,
    tab_id: impl Into<String>,
    removed_state: bool,
    active_tab_id: Option<String>,
    remaining_tab_count: usize,
) -> Result<Value, CliError> {
    to_value(TabCloseResponse {
        session_id: session_id.into(),
        tab_id: tab_id.into(),
        removed: true,
        removed_state,
        active_tab_id,
        remaining_tab_count,
    })
}

pub(crate) fn present_session_close(
    session_id: impl Into<String>,
    removed_tabs: usize,
) -> Result<Value, CliError> {
    to_value(SessionCloseResponse {
        session_id: session_id.into(),
        removed: true,
        removed_tabs,
    })
}
