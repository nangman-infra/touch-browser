use serde_json::Value;

use crate::interface::deps::{
    dispatch, merge_ack_risks, AckRisk, CliCommand, CliError, ClickOptions, ExpandOptions,
    FollowOptions, PaginateOptions, PaginationDirection, SecretPrefill, SubmitOptions, TypeOptions,
};

use super::{
    daemon_state::ServeDaemonState,
    params::{json_ack_risks, json_bool, optional_json_string, required_json_string},
    presenters,
};

#[derive(Debug)]
struct ServeTabCommandContext {
    session_id: String,
    tab_id: String,
    session_file: std::path::PathBuf,
}

pub(crate) fn serve_session_follow(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let target_ref = required_json_string(params, "targetRef")?;
    let headed = json_bool(params, "headed").unwrap_or(false);
    let result = dispatch(CliCommand::Follow(FollowOptions {
        session_file: context.session_file.clone(),
        target_ref,
        headed,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
}

pub(crate) fn serve_session_click(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let target_ref = required_json_string(params, "targetRef")?;
    let headed = json_bool(params, "headed").unwrap_or(false);
    let ack_risks = json_ack_risks(params, "ackRisks")?;
    let merged_ack_risks =
        merged_ack_risks_for_session(daemon_state, &context.session_id, &ack_risks)?;
    let result = dispatch(CliCommand::Click(ClickOptions {
        session_file: context.session_file.clone(),
        target_ref,
        headed,
        ack_risks: merged_ack_risks,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
}

pub(crate) fn serve_session_type(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let target_ref = required_json_string(params, "targetRef")?;
    let value = required_json_string(params, "value")?;
    let headed = json_bool(params, "headed").unwrap_or(false);
    let sensitive = json_bool(params, "sensitive").unwrap_or(false);
    let ack_risks = json_ack_risks(params, "ackRisks")?;
    let merged_ack_risks =
        merged_ack_risks_for_session(daemon_state, &context.session_id, &ack_risks)?;
    if sensitive {
        let session = daemon_state.session_mut(&context.session_id)?;
        session
            .secret_prefills
            .insert(target_ref.clone(), value.clone());
    }
    let result = dispatch(CliCommand::Type(TypeOptions {
        session_file: context.session_file.clone(),
        target_ref,
        value,
        headed,
        sensitive,
        ack_risks: merged_ack_risks,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
}

pub(crate) fn serve_session_type_secret(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let target_ref = required_json_string(params, "targetRef")?;
    let headed = json_bool(params, "headed").unwrap_or(false);
    let ack_risks = json_ack_risks(params, "ackRisks")?;
    let merged_ack_risks =
        merged_ack_risks_for_session(daemon_state, &context.session_id, &ack_risks)?;
    let value = daemon_secret_value(daemon_state, &context.session_id, &target_ref)?;
    let result = dispatch(CliCommand::Type(TypeOptions {
        session_file: context.session_file.clone(),
        target_ref,
        value,
        headed,
        sensitive: true,
        ack_risks: merged_ack_risks,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
}

pub(crate) fn serve_session_submit(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let target_ref = required_json_string(params, "targetRef")?;
    let headed = json_bool(params, "headed").unwrap_or(false);
    let ack_risks = json_ack_risks(params, "ackRisks")?;
    let merged_ack_risks =
        merged_ack_risks_for_session(daemon_state, &context.session_id, &ack_risks)?;
    let extra_prefill = daemon_secret_prefills(daemon_state, &context.session_id)?;
    let result = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: context.session_file.clone(),
        target_ref,
        headed,
        ack_risks: merged_ack_risks,
        extra_prefill,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
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
    presenters::present_secret_store(session_id, target_ref, session.secret_prefills.len())
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
    presenters::present_secret_clear(session_id, removed, session.secret_prefills.len())
}

pub(crate) fn serve_session_paginate(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let direction = match required_json_string(params, "direction")?.as_str() {
        "next" => PaginationDirection::Next,
        "prev" => PaginationDirection::Prev,
        _ => {
            return Err(CliError::Usage(
                "serve params `direction` must be `next` or `prev`.".to_string(),
            ))
        }
    };
    let headed = json_bool(params, "headed").unwrap_or(false);
    let result = dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: context.session_file.clone(),
        direction,
        headed,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
}

pub(crate) fn serve_session_expand(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<Value, CliError> {
    let context = resolve_serve_tab_context(params, daemon_state)?;
    let target_ref = required_json_string(params, "targetRef")?;
    let headed = json_bool(params, "headed").unwrap_or(false);
    let result = dispatch(CliCommand::Expand(ExpandOptions {
        session_file: context.session_file.clone(),
        target_ref,
        headed,
    }))?;
    presenters::present_session_tab_result(context.session_id, context.tab_id, result)
}

fn resolve_serve_tab_context(
    params: &Value,
    daemon_state: &mut ServeDaemonState,
) -> Result<ServeTabCommandContext, CliError> {
    let session_id = required_json_string(params, "sessionId")?;
    let tab_id = optional_json_string(params, "tabId");
    let (resolved_tab_id, session_file) =
        daemon_state.opened_tab_file(&session_id, tab_id.as_deref())?;
    Ok(ServeTabCommandContext {
        session_id,
        tab_id: resolved_tab_id,
        session_file,
    })
}

fn merged_ack_risks_for_session(
    daemon_state: &ServeDaemonState,
    session_id: &str,
    ack_risks: &[AckRisk],
) -> Result<Vec<AckRisk>, CliError> {
    let session = daemon_state.session(session_id)?;
    Ok(merge_ack_risks(ack_risks, &session.approved_risks))
}

fn daemon_secret_prefills(
    daemon_state: &ServeDaemonState,
    session_id: &str,
) -> Result<Vec<SecretPrefill>, CliError> {
    let session = daemon_state.session(session_id)?;
    Ok(session
        .secret_prefills
        .iter()
        .map(|(target_ref, value)| SecretPrefill {
            target_ref: target_ref.clone(),
            value: value.clone(),
        })
        .collect::<Vec<_>>())
}

fn daemon_secret_value(
    daemon_state: &ServeDaemonState,
    session_id: &str,
    target_ref: &str,
) -> Result<String, CliError> {
    let session = daemon_state.session(session_id)?;
    session
        .secret_prefills
        .get(target_ref)
        .cloned()
        .ok_or_else(|| {
            CliError::Usage(format!(
                "No daemon secret is stored for target ref `{target_ref}`."
            ))
        })
}
