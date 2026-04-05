use serde_json::{json, Value};

use crate::*;

use super::{
    daemon_state::ServeDaemonState,
    params::{
        json_bool, json_string_array, json_usize, optional_json_string, required_json_string,
    },
    session_handlers::{serve_session_open_internal, ServeSessionOpenRequest},
};

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
