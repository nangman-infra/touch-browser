use serde_json::Value;

use crate::interface::deps::{
    dispatch, load_browser_cli_session, parse_search_engine, CliCommand, CliError, SearchEngine,
    SearchOptions, SearchReportStatus, SourceRisk, DEFAULT_SEARCH_TOKENS,
};

use super::{
    daemon_state::ServeDaemonState,
    params::{
        ensure_research_headed_allowed, json_bool, json_usize, optional_json_string,
        required_json_string,
    },
    presenters,
    session_handlers::{serve_session_open_internal, ServeSessionOpenRequest},
};

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
    let engine_explicit = params.get("engine").is_some();
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
    ensure_research_headed_allowed(headed, "runtime.search")?;
    let result = dispatch(CliCommand::Search(SearchOptions {
        query,
        engine,
        engine_explicit,
        budget,
        headed,
        profile_dir: None,
        session_file: Some(session_file),
    }))?;
    daemon_state.mark_latest_search_tab(&session_id, &resolved_tab_id)?;
    presenters::present_session_tab_result(session_id, resolved_tab_id, result)
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
    let prefer_official = json_bool(params, "preferOfficial").unwrap_or(false);
    let headed = json_bool(params, "headed");
    ensure_research_headed_allowed(headed.unwrap_or(false), "runtime.search.openResult")?;
    let (resolved_search_tab_id, search_session_file) =
        daemon_state.latest_search_tab_file(&session_id, search_tab_id.as_deref())?;
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
    let (selected, selection_strategy) =
        crate::application::research_commands::selected_search_result(
            &latest_search,
            rank,
            prefer_official,
        )?;
    let target_tab_id = daemon_state.create_tab_for_session(&session_id)?;
    daemon_state.select_tab(&session_id, &target_tab_id)?;
    let open_result = serve_session_open_internal(
        daemon_state,
        ServeSessionOpenRequest {
            session_id: session_id.clone(),
            requested_tab_id: Some(target_tab_id.clone()),
            target: selected.url.clone(),
            budget: persisted.requested_budget,
            source_risk: Some(SourceRisk::Low),
            source_label: None,
            new_allowlisted_domains: Vec::new(),
            headed,
            headed_operation: "runtime.search.openResult",
            browser: true,
        },
    )?;

    presenters::present_search_open_result(
        session_id,
        resolved_search_tab_id,
        target_tab_id,
        selection_strategy,
        selected,
        open_result,
    )
}

pub(crate) fn serve_search_open_top(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let search_tab_id = optional_json_string(params, "tabId");
    let limit = json_usize(params, "limit").unwrap_or(3).max(1);
    let headed = json_bool(params, "headed");
    ensure_research_headed_allowed(headed.unwrap_or(false), "runtime.search.openTop")?;
    let (resolved_search_tab_id, search_session_file) =
        daemon_state.latest_search_tab_file(&session_id, search_tab_id.as_deref())?;
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
                budget: persisted.requested_budget,
                source_risk: Some(SourceRisk::Low),
                source_label: None,
                new_allowlisted_domains: Vec::new(),
                headed,
                headed_operation: "runtime.search.openTop",
                browser: true,
            },
        )?;
        opened_tabs.push((tab_id, selected, open_result));
    }

    presenters::present_search_open_top(session_id, resolved_search_tab_id, opened_tabs)
}
