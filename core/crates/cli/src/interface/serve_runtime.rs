use std::{
    collections::{BTreeMap, BTreeSet},
    io::{self, BufRead, Write},
    path::PathBuf,
};

use crate::{application, *};
use serde_json::json;

pub(crate) fn handle_serve() -> Result<(), CliError> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut daemon_state = ServeDaemonState::new()?;

    let serve_result = (|| -> Result<(), CliError> {
        for line in stdin.lock().lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let response = match serde_json::from_str::<ServeJsonRpcRequest>(trimmed) {
                Ok(request) => serve_dispatch(request, &mut daemon_state),
                Err(error) => serve_error(
                    Value::Null,
                    -32700,
                    format!("Invalid JSON-RPC request: {error}"),
                ),
            };

            writeln!(
                stdout,
                "{}",
                serde_json::to_string(&response).expect("serve response should serialize")
            )?;
            stdout.flush()?;
        }

        Ok(())
    })();

    let cleanup_result = daemon_state.cleanup();

    match (serve_result, cleanup_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), _) => Err(error),
        (Ok(()), Err(error)) => Err(error),
    }
}

pub(crate) fn serve_dispatch(
    request: ServeJsonRpcRequest,
    daemon_state: &mut ServeDaemonState,
) -> Value {
    let ServeJsonRpcRequest {
        id, method, params, ..
    } = request;

    let result =
        match method.as_str() {
            "runtime.status" => Ok(json!({
                "status": "ready",
                "transport": "stdio-json-rpc",
                "version": CONTRACT_VERSION,
                "daemon": true,
                "methods": [
                    "runtime.status",
                    "runtime.open",
                    "runtime.readView",
                    "runtime.extract",
                    "runtime.policy",
                    "runtime.compactView",
                    "runtime.search",
                    "runtime.search.openResult",
                    "runtime.search.openTop",
                    "runtime.session.create",
                    "runtime.session.open",
                    "runtime.session.snapshot",
                    "runtime.session.compactView",
                    "runtime.session.readView",
                    "runtime.session.refresh",
                    "runtime.session.extract",
                    "runtime.session.checkpoint",
                    "runtime.session.policy",
                    "runtime.session.profile.get",
                    "runtime.session.profile.set",
                    "runtime.session.synthesize",
                    "runtime.session.approve",
                    "runtime.session.follow",
                    "runtime.session.click",
                    "runtime.session.type",
                    "runtime.session.typeSecret",
                    "runtime.session.submit",
                    "runtime.session.secret.store",
                    "runtime.session.secret.clear",
                    "runtime.session.paginate",
                    "runtime.session.expand",
                    "runtime.session.replay",
                    "runtime.session.close",
                    "runtime.telemetry.summary",
                    "runtime.telemetry.recent",
                    "runtime.tab.open",
                    "runtime.tab.list",
                    "runtime.tab.select",
                    "runtime.tab.close"
                ]
            })),
            "runtime.open" => {
                json_target_options(&params).and_then(|options| dispatch(CliCommand::Open(options)))
            }
            "runtime.readView" => json_target_options(&params)
                .and_then(|options| dispatch(CliCommand::ReadView(options))),
            "runtime.extract" => json_extract_options(&params)
                .and_then(|options| dispatch(CliCommand::Extract(options))),
            "runtime.policy" => json_target_options(&params)
                .and_then(|options| dispatch(CliCommand::Policy(options))),
            "runtime.compactView" => json_target_options(&params)
                .and_then(|options| dispatch(CliCommand::CompactView(options))),
            "runtime.search" => serve_search(&params, daemon_state),
            "runtime.search.openResult" => serve_search_open_result(&params, daemon_state),
            "runtime.search.openTop" => serve_search_open_top(&params, daemon_state),
            "runtime.session.create" => serve_session_create(&params, daemon_state),
            "runtime.session.open" => serve_session_open(&params, daemon_state),
            "runtime.session.snapshot" => serve_session_snapshot(&params, daemon_state),
            "runtime.session.compactView" => serve_session_compact_view(&params, daemon_state),
            "runtime.session.readView" => serve_session_read_view(&params, daemon_state),
            "runtime.session.refresh" => serve_session_refresh(&params, daemon_state),
            "runtime.session.extract" => serve_session_extract(&params, daemon_state),
            "runtime.session.checkpoint" => serve_session_checkpoint(&params, daemon_state),
            "runtime.session.policy" => serve_session_policy(&params, daemon_state),
            "runtime.session.profile.get" => serve_session_profile_get(&params, daemon_state),
            "runtime.session.profile.set" => serve_session_profile_set(&params, daemon_state),
            "runtime.session.synthesize" => {
                if params.get("sessionId").is_some() {
                    serve_session_synthesize(&params, daemon_state)
                } else {
                    json_session_synthesize_options(&params)
                        .and_then(|options| dispatch(CliCommand::SessionSynthesize(options)))
                }
            }
            "runtime.session.approve" => serve_session_approve(&params, daemon_state),
            "runtime.session.follow" => serve_session_follow(&params, daemon_state),
            "runtime.session.click" => serve_session_click(&params, daemon_state),
            "runtime.session.type" => serve_session_type(&params, daemon_state),
            "runtime.session.typeSecret" => serve_session_type_secret(&params, daemon_state),
            "runtime.session.submit" => serve_session_submit(&params, daemon_state),
            "runtime.session.secret.store" => serve_session_secret_store(&params, daemon_state),
            "runtime.session.secret.clear" => serve_session_secret_clear(&params, daemon_state),
            "runtime.session.paginate" => serve_session_paginate(&params, daemon_state),
            "runtime.session.expand" => serve_session_expand(&params, daemon_state),
            "runtime.session.replay" => serve_session_replay(&params, daemon_state),
            "runtime.session.close" => {
                if params.get("sessionId").is_some() {
                    serve_session_close(&params, daemon_state)
                } else {
                    json_session_file_options(&params)
                        .and_then(|options| dispatch(CliCommand::SessionClose(options)))
                }
            }
            "runtime.tab.open" => serve_tab_open(&params, daemon_state),
            "runtime.tab.list" => serve_tab_list(&params, daemon_state),
            "runtime.tab.select" => serve_tab_select(&params, daemon_state),
            "runtime.tab.close" => serve_tab_close(&params, daemon_state),
            "runtime.telemetry.summary" => handle_telemetry_summary(),
            "runtime.telemetry.recent" => {
                let limit = json_usize(&params, "limit").unwrap_or(10);
                handle_telemetry_recent(TelemetryRecentOptions { limit })
            }
            _ => Err(CliError::Usage(format!(
                "Unsupported serve method `{method}`."
            ))),
        };

    match result {
        Ok(result) => {
            let _ =
                log_telemetry_success(&telemetry_surface_label("serve"), &method, &result, &params);
            serve_result(id, result)
        }
        Err(error) => {
            let _ = log_telemetry_error(
                &telemetry_surface_label("serve"),
                &method,
                &error.to_string(),
                params.get("sessionId").and_then(Value::as_str),
                &params,
            );
            serve_error(id, -32602, error.to_string())
        }
    }
}

pub(crate) fn serve_result(id: Value, result: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": result
    })
}

pub(crate) fn serve_error(id: Value, code: i64, message: String) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {
            "code": code,
            "message": message
        }
    })
}

pub(crate) fn json_target_options(params: &Value) -> Result<TargetOptions, CliError> {
    Ok(TargetOptions {
        target: required_json_string(params, "target")?,
        budget: json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS),
        source_risk: optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?,
        source_label: optional_json_string(params, "sourceLabel"),
        allowlisted_domains: json_string_array(params, "allowDomains")?,
        browser: json_bool(params, "browser").unwrap_or(false),
        headed: json_bool(params, "headed").unwrap_or(false),
        main_only: json_bool(params, "mainOnly").unwrap_or(false),
        session_file: optional_json_string(params, "sessionFile").map(PathBuf::from),
    })
}

pub(crate) fn json_extract_options(params: &Value) -> Result<ExtractOptions, CliError> {
    let claims = json_string_array(params, "claims")?;
    if claims.is_empty() {
        return Err(CliError::Usage(
            "serve params `claims` must include at least one statement.".to_string(),
        ));
    }

    Ok(ExtractOptions {
        target: required_json_string(params, "target")?,
        budget: json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS),
        source_risk: optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?,
        source_label: optional_json_string(params, "sourceLabel"),
        allowlisted_domains: json_string_array(params, "allowDomains")?,
        browser: json_bool(params, "browser").unwrap_or(false),
        headed: json_bool(params, "headed").unwrap_or(false),
        session_file: optional_json_string(params, "sessionFile").map(PathBuf::from),
        claims,
        verifier_command: optional_json_string(params, "verifierCommand"),
    })
}

pub(crate) fn json_session_file_options(params: &Value) -> Result<SessionFileOptions, CliError> {
    Ok(SessionFileOptions {
        session_file: PathBuf::from(required_json_string(params, "sessionFile")?),
    })
}

pub(crate) fn json_session_synthesize_options(
    params: &Value,
) -> Result<SessionSynthesizeOptions, CliError> {
    Ok(SessionSynthesizeOptions {
        session_file: PathBuf::from(required_json_string(params, "sessionFile")?),
        note_limit: json_usize(params, "noteLimit").unwrap_or(12),
        format: optional_json_string(params, "format")
            .map(|value| parse_output_format(&value))
            .transpose()?
            .unwrap_or(OutputFormat::Json),
    })
}

pub(crate) fn required_json_string(params: &Value, field: &str) -> Result<String, CliError> {
    optional_json_string(params, field)
        .ok_or_else(|| CliError::Usage(format!("serve params require `{field}` as a string.")))
}

pub(crate) fn optional_json_string(params: &Value, field: &str) -> Option<String> {
    params
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn json_string_array(params: &Value, field: &str) -> Result<Vec<String>, CliError> {
    match params.get(field) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(ToString::to_string).ok_or_else(|| {
                    CliError::Usage(format!(
                        "serve params `{field}` must be an array of strings."
                    ))
                })
            })
            .collect(),
        Some(_) => Err(CliError::Usage(format!(
            "serve params `{field}` must be an array of strings."
        ))),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn json_ack_risks(params: &Value, field: &str) -> Result<Vec<AckRisk>, CliError> {
    json_string_array(params, field)?
        .into_iter()
        .map(|value| parse_ack_risk(&value))
        .collect()
}

pub(crate) fn json_bool(params: &Value, field: &str) -> Option<bool> {
    params.get(field).and_then(Value::as_bool)
}

pub(crate) fn json_usize(params: &Value, field: &str) -> Option<usize> {
    params
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

pub(crate) fn serve_search(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let requested_tab_id = optional_json_string(params, "tabId");
    let query = required_json_string(params, "query")?;
    let engine = optional_json_string(params, "engine")
        .map(|value| parse_search_engine(&value))
        .transpose()?
        .unwrap_or(SearchEngine::Google);
    let budget = json_usize(params, "budget").unwrap_or(DEFAULT_SEARCH_TOKENS);
    let resolved_tab_id = match requested_tab_id.as_deref() {
        Some(tab_id) => {
            daemon_state.ensure_tab(&session_id, tab_id)?;
            daemon_state.select_tab(&session_id, tab_id)?;
            tab_id.to_string()
        }
        None => daemon_state.ensure_active_tab(&session_id)?,
    };
    let (headless, session_file) = {
        let session = daemon_state.session(&session_id)?;
        let tab = session.tabs.get(&resolved_tab_id).ok_or_else(|| {
            CliError::Usage(format!(
                "Serve session `{session_id}` does not contain tab `{resolved_tab_id}`."
            ))
        })?;
        (session.headless, tab.session_file.clone())
    };
    let headed = json_bool(params, "headed").unwrap_or(!headless);
    let result = dispatch(CliCommand::Search(SearchOptions {
        query,
        engine,
        budget,
        headed,
        profile_dir: None,
        session_file: Some(session_file),
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_search_open_result(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let search_tab_id = optional_json_string(params, "tabId");
    let rank = json_usize(params, "rank").ok_or_else(|| {
        CliError::Usage("serve params `rank` must be a positive number.".to_string())
    })?;
    if rank == 0 {
        return Err(CliError::Usage(
            "serve params `rank` must be a positive number.".to_string(),
        ));
    }
    let headed = json_bool(params, "headed");
    let (resolved_search_tab_id, search_session_file) =
        daemon_state.opened_tab_file(&session_id, search_tab_id.as_deref())?;
    let persisted = load_browser_cli_session(&search_session_file)?;
    let latest_search = persisted.latest_search.ok_or_else(|| {
        CliError::Usage(format!(
            "Tab `{resolved_search_tab_id}` does not contain saved search results."
        ))
    })?;
    if latest_search.status != SearchReportStatus::Ready {
        return Err(CliError::Usage(
            latest_search
                .status_detail
                .clone()
                .unwrap_or_else(|| "Saved search results are not ready to open.".to_string()),
        ));
    }
    let selected = latest_search
        .results
        .iter()
        .find(|result| result.rank == rank)
        .cloned()
        .ok_or_else(|| CliError::Usage(format!("Search results do not contain rank {rank}.")))?;
    let target_tab_id = daemon_state.create_tab_for_session(&session_id)?;
    daemon_state.select_tab(&session_id, &target_tab_id)?;
    let open_result = serve_session_open_internal(
        daemon_state,
        ServeSessionOpenRequest {
            session_id: session_id.clone(),
            requested_tab_id: Some(target_tab_id.clone()),
            target: selected.url.clone(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: Some(SourceRisk::Low),
            source_label: None,
            new_allowlisted_domains: Vec::new(),
            headed,
            browser: true,
        },
    )?;

    Ok(json!({
        "sessionId": session_id,
        "searchTabId": resolved_search_tab_id,
        "openedTabId": target_tab_id,
        "selectedResult": selected,
        "result": open_result,
    }))
}

pub(crate) fn serve_search_open_top(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let search_tab_id = optional_json_string(params, "tabId");
    let limit = json_usize(params, "limit").unwrap_or(3).max(1);
    let headed = json_bool(params, "headed");
    let (resolved_search_tab_id, search_session_file) =
        daemon_state.opened_tab_file(&session_id, search_tab_id.as_deref())?;
    let persisted = load_browser_cli_session(&search_session_file)?;
    let latest_search = persisted.latest_search.ok_or_else(|| {
        CliError::Usage(format!(
            "Tab `{resolved_search_tab_id}` does not contain saved search results."
        ))
    })?;
    if latest_search.status != SearchReportStatus::Ready {
        return Err(CliError::Usage(
            latest_search
                .status_detail
                .clone()
                .unwrap_or_else(|| "Saved search results are not ready to open.".to_string()),
        ));
    }

    let selected_ranks = if latest_search.recommended_result_ranks.is_empty() {
        latest_search
            .results
            .iter()
            .map(|result| result.rank)
            .take(limit)
            .collect::<Vec<_>>()
    } else {
        latest_search
            .recommended_result_ranks
            .iter()
            .copied()
            .take(limit)
            .collect::<Vec<_>>()
    };

    let mut opened_tabs = Vec::new();
    for rank in selected_ranks {
        let selected = latest_search
            .results
            .iter()
            .find(|result| result.rank == rank)
            .cloned()
            .ok_or_else(|| {
                CliError::Usage(format!("Search results do not contain rank {rank}."))
            })?;
        let tab_id = daemon_state.create_tab_for_session(&session_id)?;
        let open_result = serve_session_open_internal(
            daemon_state,
            ServeSessionOpenRequest {
                session_id: session_id.clone(),
                requested_tab_id: Some(tab_id.clone()),
                target: selected.url.clone(),
                budget: DEFAULT_REQUESTED_TOKENS,
                source_risk: Some(SourceRisk::Low),
                source_label: None,
                new_allowlisted_domains: Vec::new(),
                headed,
                browser: true,
            },
        )?;
        opened_tabs.push(json!({
            "tabId": tab_id,
            "selectedResult": selected,
            "result": open_result,
        }));
    }

    Ok(json!({
        "sessionId": session_id,
        "searchTabId": resolved_search_tab_id,
        "openedCount": opened_tabs.len(),
        "openedTabs": opened_tabs,
    }))
}

pub(crate) fn serve_session_create(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let headless = !json_bool(params, "headed").unwrap_or(false);
    let allowlisted_domains = json_string_array(params, "allowDomains")?;
    let (session_id, active_tab_id) =
        daemon_state.create_session(headless, allowlisted_domains.clone())?;
    Ok(json!({
        "sessionId": session_id,
        "activeTabId": active_tab_id,
        "headless": headless,
        "allowDomains": allowlisted_domains,
        "tabCount": 1,
    }))
}

pub(crate) fn serve_session_open(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let target = required_json_string(params, "target")?;
    let source_risk = optional_json_string(params, "sourceRisk")
        .map(|value| parse_source_risk(&value))
        .transpose()?;
    let source_label = optional_json_string(params, "sourceLabel");
    let allowlisted_domains = json_string_array(params, "allowDomains")?;
    let headed = json_bool(params, "headed");
    let browser = json_bool(params, "browser").unwrap_or(true);
    let budget = json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS);

    serve_session_open_internal(
        daemon_state,
        ServeSessionOpenRequest {
            session_id,
            requested_tab_id: tab_id,
            target,
            budget,
            source_risk,
            source_label,
            new_allowlisted_domains: allowlisted_domains,
            headed,
            browser,
        },
    )
}

pub(crate) fn serve_session_snapshot(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionSnapshot(SessionFileOptions {
        session_file,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_compact_view(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionCompact(SessionFileOptions {
        session_file,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_read_view(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let main_only = json_bool(params, "mainOnly").unwrap_or(false);
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionRead(SessionReadOptions {
        session_file,
        main_only,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_refresh(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let headed = json_bool(params, "headed").unwrap_or(false);
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionRefresh(SessionRefreshOptions {
        session_file,
        headed,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_extract(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let claims = json_string_array(params, "claims")?;
    if claims.is_empty() {
        return Err(CliError::Usage(
            "serve params `claims` must include at least one statement.".to_string(),
        ));
    }
    let verifier_command = optional_json_string(params, "verifierCommand");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file,
        claims,
        verifier_command,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_policy(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionPolicy(SessionFileOptions {
        session_file,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_profile_get(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SessionProfile(SessionFileOptions {
        session_file,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_profile_set(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let profile_value = required_json_string(params, "profile")?;
    let profile = parse_policy_profile(&profile_value)?;
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::SetProfile(SessionProfileSetOptions {
        session_file,
        profile,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_checkpoint(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let mut result = dispatch(CliCommand::SessionCheckpoint(SessionFileOptions {
        session_file,
    }))?;
    let approved_risks = {
        let session = daemon_state.session(&session_id)?;
        approved_risk_labels(&session.approved_risks)
    };
    result["checkpoint"]["approvedRisks"] = json!(approved_risks);
    result["checkpoint"]["approvalPanel"]["approvedRisks"] =
        result["checkpoint"]["approvedRisks"].clone();
    result["checkpoint"]["playbook"]["approvedRisks"] =
        result["checkpoint"]["approvedRisks"].clone();
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_synthesize(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let note_limit = json_usize(params, "noteLimit").unwrap_or(12);
    let format = optional_json_string(params, "format")
        .map(|value| parse_output_format(&value))
        .transpose()?
        .unwrap_or(OutputFormat::Json);
    let session = daemon_state.session(&session_id)?;

    let runtime = ReadOnlyRuntime::default();
    let mut tab_reports = Vec::new();

    for (tab_id, tab) in &session.tabs {
        if !tab.session_file.is_file() {
            continue;
        }
        let persisted = load_browser_cli_session(&tab.session_file)?;
        let report = runtime.synthesize_session(
            &persisted.session,
            &persisted.session.state.updated_at,
            note_limit,
        )?;
        tab_reports.push((tab_id.clone(), report));
    }

    if tab_reports.is_empty() {
        return Err(CliError::Usage(format!(
            "Serve session `{session_id}` has no opened tabs to synthesize."
        )));
    }

    let report = combine_session_synthesis_reports(&session_id, note_limit, &tab_reports);
    let tab_reports_json = tab_reports
        .into_iter()
        .map(|(tab_id, report)| {
            json!({
                "tabId": tab_id,
                "report": report,
            })
        })
        .collect::<Vec<_>>();

    Ok(json!({
        "sessionId": session_id,
        "activeTabId": session.active_tab_id,
        "tabCount": session.tabs.len(),
        "format": format,
        "markdown": (format == OutputFormat::Markdown).then(|| render_session_synthesis_markdown(&report)),
        "report": report,
        "tabReports": tab_reports_json,
    }))
}

pub(crate) fn serve_session_approve(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let ack_risks = json_ack_risks(params, "ackRisks")?;
    if ack_risks.is_empty() {
        return Err(CliError::Usage(
            "serve params `ackRisks` must include at least one approval risk.".to_string(),
        ));
    }

    let session = daemon_state.session_mut(&session_id)?;
    for ack_risk in ack_risks {
        session.approved_risks.insert(ack_risk);
    }
    let promoted_profile = promoted_policy_profile_for_risks(
        PolicyProfile::InteractiveReview,
        &session.approved_risks,
    );
    for tab in session.tabs.values() {
        if !tab.session_file.is_file() {
            continue;
        }
        let mut persisted = load_browser_cli_session(&tab.session_file)?;
        persisted.session.state.policy_profile = promoted_profile;
        save_browser_cli_session(&tab.session_file, &persisted)?;
    }

    Ok(json!({
        "sessionId": session_id,
        "approvedRisks": approved_risk_labels(&session.approved_risks),
        "policyProfile": policy_profile_label(promoted_profile),
    }))
}

pub(crate) fn serve_session_follow(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_follow(params, daemon_state)
}

pub(crate) fn serve_session_click(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_click(params, daemon_state)
}

pub(crate) fn serve_session_type(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_type(params, daemon_state)
}

pub(crate) fn serve_session_type_secret(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_type_secret(params, daemon_state)
}

pub(crate) fn serve_session_submit(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_submit(params, daemon_state)
}

pub(crate) fn serve_session_secret_store(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let target_ref = required_json_string(params, "targetRef")?;
    let value = required_json_string(params, "value")?;
    let session = daemon_state.session_mut(&session_id)?;
    session.secret_prefills.insert(target_ref.clone(), value);
    Ok(json!({
        "sessionId": session_id,
        "stored": true,
        "targetRef": target_ref,
        "secretCount": session.secret_prefills.len(),
    }))
}

pub(crate) fn serve_session_secret_clear(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let target_ref = optional_json_string(params, "targetRef");
    let session = daemon_state.session_mut(&session_id)?;
    let removed = match target_ref {
        Some(target_ref) => session.secret_prefills.remove(&target_ref).is_some(),
        None => {
            let had_any = !session.secret_prefills.is_empty();
            session.secret_prefills.clear();
            had_any
        }
    };
    Ok(json!({
        "sessionId": session_id,
        "removed": removed,
        "secretCount": session.secret_prefills.len(),
    }))
}

pub(crate) fn serve_session_paginate(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_paginate(params, daemon_state)
}

pub(crate) fn serve_session_expand(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    application::browser_session_actions::serve_session_expand(params, daemon_state)
}

pub(crate) fn serve_session_replay(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    let result = dispatch(CliCommand::BrowserReplay(SessionFileOptions {
        session_file,
    }))?;
    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn serve_session_close(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    daemon_state.close_session(&session_id)
}

pub(crate) fn serve_tab_open(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = daemon_state.create_tab_for_session(&session_id)?;
    daemon_state.select_tab(&session_id, &tab_id)?;

    if let Some(target) = optional_json_string(params, "target") {
        let source_risk = optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?;
        let source_label = optional_json_string(params, "sourceLabel");
        let allowlisted_domains = json_string_array(params, "allowDomains")?;
        let headed = json_bool(params, "headed");
        let browser = json_bool(params, "browser").unwrap_or(true);
        let budget = json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS);
        return serve_session_open_internal(
            daemon_state,
            ServeSessionOpenRequest {
                session_id,
                requested_tab_id: Some(tab_id),
                target,
                budget,
                source_risk,
                source_label,
                new_allowlisted_domains: allowlisted_domains,
                headed,
                browser,
            },
        );
    }

    Ok(json!({
        "sessionId": session_id,
        "activeTabId": tab_id,
        "tab": daemon_state.tab_summary(&session_id, &tab_id)?,
    }))
}

pub(crate) fn serve_tab_list(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let session = daemon_state.session(&session_id)?;
    let tabs = session
        .tabs
        .keys()
        .map(|tab_id| daemon_state.tab_summary(&session_id, tab_id))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({
        "sessionId": session_id,
        "activeTabId": session.active_tab_id,
        "tabs": tabs,
    }))
}

pub(crate) fn serve_tab_select(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = required_json_string(params, "tabId")?;
    daemon_state.select_tab(&session_id, &tab_id)?;
    Ok(json!({
        "sessionId": session_id,
        "activeTabId": tab_id.clone(),
        "tab": daemon_state.tab_summary(&session_id, &tab_id)?,
    }))
}

pub(crate) fn serve_tab_close(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = required_json_string(params, "tabId")?;
    daemon_state.close_tab(&session_id, &tab_id)
}

pub(crate) fn serve_session_open_internal(
    daemon_state: &mut ServeDaemonState,
    request: ServeSessionOpenRequest,
) -> Result<Value, CliError> {
    let ServeSessionOpenRequest {
        session_id,
        requested_tab_id,
        target,
        budget,
        source_risk,
        source_label,
        new_allowlisted_domains,
        headed,
        browser,
    } = request;

    if !browser {
        return Err(CliError::Usage(
            "Serve daemon sessions currently require browser-backed open.".to_string(),
        ));
    }

    let resolved_tab_id = match requested_tab_id.as_deref() {
        Some(tab_id) => {
            daemon_state.ensure_tab(&session_id, tab_id)?;
            daemon_state.select_tab(&session_id, tab_id)?;
            tab_id.to_string()
        }
        None => daemon_state.ensure_active_tab(&session_id)?,
    };

    daemon_state.extend_session_allowlist(&session_id, &new_allowlisted_domains)?;
    let (headless, allowlisted_domains, session_file) = {
        let session = daemon_state.session(&session_id)?;
        let tab = session.tabs.get(&resolved_tab_id).ok_or_else(|| {
            CliError::Usage(format!(
                "Serve session `{session_id}` does not contain tab `{resolved_tab_id}`."
            ))
        })?;
        (
            session.headless,
            session.allowlisted_domains.clone(),
            tab.session_file.clone(),
        )
    };

    let result = dispatch(CliCommand::Open(TargetOptions {
        target,
        budget,
        source_risk,
        source_label,
        allowlisted_domains,
        browser: true,
        headed: headed.unwrap_or(!headless),
        main_only: false,
        session_file: Some(session_file),
    }))?;

    Ok(json!({
        "sessionId": session_id,
        "tabId": resolved_tab_id,
        "result": result,
    }))
}

pub(crate) fn combine_session_synthesis_reports(
    session_id: &str,
    note_limit: usize,
    reports: &[(String, SessionSynthesisReport)],
) -> SessionSynthesisReport {
    #[derive(Debug, Clone)]
    struct AggregateClaim {
        claim_id: String,
        statement: String,
        status: SessionSynthesisClaimStatus,
        snapshot_ids: BTreeSet<String>,
        support_refs: BTreeSet<String>,
        citations: Vec<EvidenceCitation>,
        citation_keys: BTreeSet<String>,
    }

    fn citation_key(citation: &EvidenceCitation) -> String {
        format!(
            "{}|{}|{:?}|{:?}|{}",
            citation.url,
            citation.retrieved_at,
            citation.source_type,
            citation.source_risk,
            citation.source_label.clone().unwrap_or_default()
        )
    }

    fn merge_claim(
        aggregates: &mut BTreeMap<(String, String), AggregateClaim>,
        claim: &SessionSynthesisClaim,
        status: SessionSynthesisClaimStatus,
    ) {
        let key = (claim.claim_id.clone(), claim.statement.clone());
        let aggregate = aggregates.entry(key).or_insert_with(|| AggregateClaim {
            claim_id: claim.claim_id.clone(),
            statement: claim.statement.clone(),
            status,
            snapshot_ids: BTreeSet::new(),
            support_refs: BTreeSet::new(),
            citations: Vec::new(),
            citation_keys: BTreeSet::new(),
        });

        aggregate
            .snapshot_ids
            .extend(claim.snapshot_ids.iter().cloned());
        aggregate
            .support_refs
            .extend(claim.support_refs.iter().cloned());
        for citation in &claim.citations {
            let key = citation_key(citation);
            if aggregate.citation_keys.insert(key) {
                aggregate.citations.push(citation.clone());
            }
        }
    }

    let mut visited_urls = BTreeSet::new();
    let mut working_set_refs = BTreeSet::new();
    let mut synthesized_notes = Vec::new();
    let mut note_keys = BTreeSet::new();
    let mut supported = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut contradicted = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut unsupported = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut needs_more_browsing = BTreeMap::<(String, String), AggregateClaim>::new();
    let mut snapshot_count = 0usize;
    let mut evidence_report_count = 0usize;
    let mut generated_at = DEFAULT_OPENED_AT.to_string();

    for (_, report) in reports {
        snapshot_count += report.snapshot_count;
        evidence_report_count += report.evidence_report_count;
        generated_at = report.generated_at.clone();
        visited_urls.extend(report.visited_urls.iter().cloned());
        working_set_refs.extend(report.working_set_refs.iter().cloned());
        for note in &report.synthesized_notes {
            if note_keys.insert(note.clone()) && synthesized_notes.len() < note_limit {
                synthesized_notes.push(note.clone());
            }
        }
        for claim in &report.supported_claims {
            merge_claim(
                &mut supported,
                claim,
                SessionSynthesisClaimStatus::EvidenceSupported,
            );
        }
        for claim in &report.contradicted_claims {
            merge_claim(
                &mut contradicted,
                claim,
                SessionSynthesisClaimStatus::Contradicted,
            );
        }
        for claim in &report.unsupported_claims {
            merge_claim(
                &mut unsupported,
                claim,
                SessionSynthesisClaimStatus::InsufficientEvidence,
            );
        }
        for claim in &report.needs_more_browsing_claims {
            merge_claim(
                &mut needs_more_browsing,
                claim,
                SessionSynthesisClaimStatus::NeedsMoreBrowsing,
            );
        }
    }

    let into_claims = |aggregates: BTreeMap<(String, String), AggregateClaim>| {
        aggregates
            .into_values()
            .map(|aggregate| SessionSynthesisClaim {
                version: CONTRACT_VERSION.to_string(),
                claim_id: aggregate.claim_id,
                statement: aggregate.statement,
                status: aggregate.status,
                snapshot_ids: aggregate.snapshot_ids.into_iter().collect(),
                support_refs: aggregate.support_refs.into_iter().collect(),
                citations: aggregate.citations,
            })
            .collect::<Vec<_>>()
    };

    SessionSynthesisReport {
        version: CONTRACT_VERSION.to_string(),
        session_id: session_id.to_string(),
        generated_at,
        snapshot_count,
        evidence_report_count,
        visited_urls: visited_urls.into_iter().collect(),
        working_set_refs: working_set_refs.into_iter().collect(),
        synthesized_notes,
        supported_claims: into_claims(supported),
        contradicted_claims: into_claims(contradicted),
        unsupported_claims: into_claims(unsupported),
        needs_more_browsing_claims: into_claims(needs_more_browsing),
    }
}
