use std::path::PathBuf;

use serde_json::{json, Value};

use crate::interface::serve_runtime::{
    json_ack_risks, json_bool, optional_json_string, required_json_string,
};
use crate::*;

#[derive(Debug)]
struct ServeTabCommandContext {
    session_id: String,
    tab_id: String,
    session_file: PathBuf,
}

pub(crate) fn handle_follow(options: FollowOptions) -> Result<Value, CliError> {
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = load_browser_cli_session(&options.session_file)?;
    if let Some(rejected) = preflight_ref_action(
        &persisted,
        &kernel,
        ActionName::Follow,
        &options.target_ref,
        "Follow target is blocked by the current policy boundary.",
        &options.session_file,
    ) {
        return Ok(json!(rejected));
    }
    let source = current_browser_action_source(&persisted)?;
    let target_text = current_snapshot_ref_text(&persisted.session, &options.target_ref)?;
    let target_href = current_snapshot_ref_href(&persisted.session, &options.target_ref);
    let target_tag_name = current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_dom_path_hint =
        current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = stable_ref_ordinal_hint(&options.target_ref);
    let result = invoke_playwright_follow(PlaywrightFollowParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        target_ref: options.target_ref.clone(),
        target_text,
        target_href,
        target_tag_name,
        target_dom_path_hint,
        target_ordinal_hint,
        headless: !options.headed,
    })?;
    let source_url = resolved_browser_source_url(&source, &result.final_url);
    let snapshot = compile_browser_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "follow".to_string(),
        timestamp: timestamp.clone(),
        target_ref: Some(options.target_ref.clone()),
        direction: None,
        text_value: None,
        redacted: false,
    });
    save_browser_cli_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Follow,
        "browser-action-result",
        json!({
            "snapshot": snapshot,
            "adapter": {
                "followedRef": result.followed_ref,
                "targetText": result.target_text,
                "targetHref": result.target_href,
                "clickedText": result.clicked_text,
                "title": result.title,
                "visibleText": result.visible_text,
                "finalUrl": result.final_url,
            }
        }),
        "Followed a link in the persisted browser session.",
        current_policy_with_allowlist(&persisted.session, &kernel, &persisted.allowlisted_domains),
    );

    Ok(json!(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    }))
}

pub(crate) fn handle_click(options: ClickOptions) -> Result<Value, CliError> {
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = load_browser_cli_session(&options.session_file)?;
    if let Some(rejected) = preflight_interactive_action(
        &persisted,
        &kernel,
        InteractivePreflightOptions {
            action: ActionName::Click,
            target_ref: Some(&options.target_ref),
            headed: options.headed,
            ack_risks: &options.ack_risks,
            message: "Click target is blocked by the current policy boundary.",
            session_file: &options.session_file,
        },
    ) {
        return Ok(json!(rejected));
    }

    let source = current_browser_action_source(&persisted)?;
    let target_text = current_snapshot_ref_text(&persisted.session, &options.target_ref)?;
    let target_href = current_snapshot_ref_href(&persisted.session, &options.target_ref);
    let target_tag_name = current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_dom_path_hint =
        current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = stable_ref_ordinal_hint(&options.target_ref);
    let result = invoke_playwright_click(PlaywrightClickParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        target_ref: options.target_ref.clone(),
        target_text,
        target_href,
        target_tag_name,
        target_dom_path_hint,
        target_ordinal_hint,
        headless: !options.headed,
    })?;
    let source_url = resolved_browser_source_url(&source, &result.final_url);
    let snapshot = compile_browser_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    mark_browser_session_interactive(&mut persisted);
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "click".to_string(),
        timestamp: timestamp.clone(),
        target_ref: Some(options.target_ref.clone()),
        direction: None,
        text_value: None,
        redacted: false,
    });
    save_browser_cli_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Click,
        "browser-action-result",
        json!({
            "snapshot": snapshot,
            "adapter": {
                "clickedRef": result.clicked_ref,
                "targetText": result.target_text,
                "targetHref": result.target_href,
                "clickedText": result.clicked_text,
                "title": result.title,
                "visibleText": result.visible_text,
                "finalUrl": result.final_url,
            }
        }),
        "Clicked an interactive target in the persisted browser session.",
        current_policy_with_allowlist(&persisted.session, &kernel, &persisted.allowlisted_domains),
    );

    Ok(json!(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    }))
}

pub(crate) fn handle_type(options: TypeOptions) -> Result<Value, CliError> {
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = load_browser_cli_session(&options.session_file)?;
    if let Some(rejected) = preflight_interactive_action(
        &persisted,
        &kernel,
        InteractivePreflightOptions {
            action: ActionName::Type,
            target_ref: Some(&options.target_ref),
            headed: options.headed,
            ack_risks: &options.ack_risks,
            message: "Type target is blocked by the current policy boundary.",
            session_file: &options.session_file,
        },
    ) {
        return Ok(json!(rejected));
    }

    if current_snapshot_ref_is_sensitive(&persisted.session, &options.target_ref)
        && !options.sensitive
    {
        let action_result = reject_action(
            ActionName::Type,
            ActionFailureKind::PolicyBlocked,
            "Sensitive credential-like inputs require explicit `--sensitive` opt-in.",
            current_policy_with_allowlist(
                &persisted.session,
                &kernel,
                &persisted.allowlisted_domains,
            ),
        );
        return Ok(json!(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state,
            session_file: options.session_file.display().to_string(),
        }));
    }

    let source = current_browser_action_source(&persisted)?;
    let target_tag_name = current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_text = if target_tag_name.as_deref() == Some("form") {
        String::new()
    } else {
        current_snapshot_ref_text(&persisted.session, &options.target_ref)?
    };
    let target_dom_path_hint =
        current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = stable_ref_ordinal_hint(&options.target_ref);
    let target_name = current_snapshot_ref_name(&persisted.session, &options.target_ref);
    let target_input_type =
        current_snapshot_ref_input_type(&persisted.session, &options.target_ref);
    let result = invoke_playwright_type(PlaywrightTypeParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        target_ref: options.target_ref.clone(),
        target_text,
        target_tag_name,
        target_dom_path_hint,
        target_ordinal_hint,
        target_name,
        target_input_type,
        value: options.value.clone(),
        headless: !options.headed,
    })?;
    let source_url = resolved_browser_source_url(&source, &result.final_url);
    let snapshot = compile_browser_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    mark_browser_session_interactive(&mut persisted);
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    if options.sensitive {
        let secret_store_path = browser_secret_store_path(&options.session_file);
        let mut secrets = load_browser_cli_secrets(&secret_store_path)?;
        secrets.insert(options.target_ref.clone(), options.value.clone());
        save_browser_cli_secrets(&secret_store_path, &secrets)?;
    }
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "type".to_string(),
        timestamp: timestamp.clone(),
        target_ref: Some(options.target_ref.clone()),
        direction: None,
        text_value: (!options.sensitive).then_some(options.value.clone()),
        redacted: options.sensitive,
    });
    save_browser_cli_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Type,
        "browser-action-result",
        json!({
            "snapshot": snapshot,
            "adapter": {
                "typedRef": result.typed_ref,
                "targetText": result.target_text,
                "typedLength": result.typed_length,
                "sensitive": options.sensitive,
                "title": result.title,
                "visibleText": result.visible_text,
                "finalUrl": result.final_url,
            }
        }),
        "Typed into an interactive field in the persisted browser session.",
        current_policy_with_allowlist(&persisted.session, &kernel, &persisted.allowlisted_domains),
    );

    Ok(json!(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    }))
}

pub(crate) fn handle_submit(options: SubmitOptions) -> Result<Value, CliError> {
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = load_browser_cli_session(&options.session_file)?;
    if let Some(rejected) = preflight_interactive_action(
        &persisted,
        &kernel,
        InteractivePreflightOptions {
            action: ActionName::Submit,
            target_ref: Some(&options.target_ref),
            headed: options.headed,
            ack_risks: &options.ack_risks,
            message: "Submit target is blocked by the current policy boundary.",
            session_file: &options.session_file,
        },
    ) {
        return Ok(json!(rejected));
    }

    let source = current_browser_action_source(&persisted)?;
    let target_tag_name = current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_text = if target_tag_name.as_deref() == Some("form") {
        String::new()
    } else {
        current_snapshot_ref_text(&persisted.session, &options.target_ref)?
    };
    let target_dom_path_hint =
        current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = stable_ref_ordinal_hint(&options.target_ref);
    let result = invoke_playwright_submit(PlaywrightSubmitParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        target_ref: options.target_ref.clone(),
        target_text,
        target_tag_name,
        target_dom_path_hint,
        target_ordinal_hint,
        prefill: collect_submit_prefill(&persisted, &{
            let mut extra_prefill = options.extra_prefill.clone();
            let secret_store_path = browser_secret_store_path(&options.session_file);
            let secrets = load_browser_cli_secrets(&secret_store_path)?;
            extra_prefill.extend(
                secrets
                    .into_iter()
                    .map(|(target_ref, value)| SecretPrefill { target_ref, value }),
            );
            extra_prefill
        }),
        headless: !options.headed,
    })?;
    let source_url = resolved_browser_source_url(&source, &result.final_url);
    let snapshot = compile_browser_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    mark_browser_session_interactive(&mut persisted);
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "submit".to_string(),
        timestamp: timestamp.clone(),
        target_ref: Some(options.target_ref.clone()),
        direction: None,
        text_value: None,
        redacted: false,
    });
    save_browser_cli_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Submit,
        "browser-action-result",
        json!({
            "snapshot": snapshot,
            "adapter": {
                "submittedRef": result.submitted_ref,
                "targetText": result.target_text,
                "title": result.title,
                "visibleText": result.visible_text,
                "finalUrl": result.final_url,
            }
        }),
        "Submitted an interactive form or submit control in the persisted browser session.",
        current_policy_with_allowlist(&persisted.session, &kernel, &persisted.allowlisted_domains),
    );

    Ok(json!(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    }))
}

pub(crate) fn handle_paginate(options: PaginateOptions) -> Result<Value, CliError> {
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = load_browser_cli_session(&options.session_file)?;
    if let Some(rejected) = preflight_session_block(
        &persisted,
        &kernel,
        ActionName::Paginate,
        "Paginate is blocked because the current snapshot requires review/block.",
        &options.session_file,
    ) {
        return Ok(json!(rejected));
    }
    let source = current_browser_action_source(&persisted)?;
    let current_page = persisted.session.snapshots.len();
    let result = invoke_playwright_paginate(PlaywrightPaginateParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        direction: match options.direction {
            PaginationDirection::Next => "next".to_string(),
            PaginationDirection::Prev => "prev".to_string(),
        },
        current_page,
        headless: !options.headed,
    })?;
    let source_url = resolved_browser_source_url(&source, &result.final_url);
    let snapshot = compile_browser_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "paginate".to_string(),
        timestamp: timestamp.clone(),
        target_ref: None,
        direction: Some(match options.direction {
            PaginationDirection::Next => "next".to_string(),
            PaginationDirection::Prev => "prev".to_string(),
        }),
        text_value: None,
        redacted: false,
    });
    save_browser_cli_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Paginate,
        "browser-action-result",
        json!({
            "snapshot": snapshot,
            "adapter": {
                "page": result.page,
                "clickedText": result.clicked_text,
                "title": result.title,
                "visibleText": result.visible_text,
                "finalUrl": result.final_url,
            }
        }),
        "Paginated persisted browser session.",
        current_policy_with_allowlist(&persisted.session, &kernel, &persisted.allowlisted_domains),
    );

    Ok(json!(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    }))
}

pub(crate) fn handle_expand(options: ExpandOptions) -> Result<Value, CliError> {
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = load_browser_cli_session(&options.session_file)?;
    if let Some(rejected) = preflight_ref_action(
        &persisted,
        &kernel,
        ActionName::Expand,
        &options.target_ref,
        "Expand target is blocked by the current policy boundary.",
        &options.session_file,
    ) {
        return Ok(json!(rejected));
    }
    let source = current_browser_action_source(&persisted)?;
    let target_text = current_snapshot_ref_text(&persisted.session, &options.target_ref)?;
    let target_tag_name = current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_dom_path_hint =
        current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = stable_ref_ordinal_hint(&options.target_ref);
    let result = invoke_playwright_expand(PlaywrightExpandParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        target_ref: options.target_ref.clone(),
        target_text,
        target_tag_name,
        target_dom_path_hint,
        target_ordinal_hint,
        headless: !options.headed,
    })?;
    let source_url = resolved_browser_source_url(&source, &result.final_url);
    let snapshot = compile_browser_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "expand".to_string(),
        timestamp: timestamp.clone(),
        target_ref: Some(options.target_ref.clone()),
        direction: None,
        text_value: None,
        redacted: false,
    });
    save_browser_cli_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Expand,
        "browser-action-result",
        json!({
            "snapshot": snapshot,
            "adapter": {
                "expandedRef": result.expanded_ref,
                "targetText": result.target_text,
                "clickedText": result.clicked_text,
                "title": result.title,
                "visibleText": result.visible_text,
                "finalUrl": result.final_url,
            }
        }),
        "Expanded persisted browser session.",
        current_policy_with_allowlist(&persisted.session, &kernel, &persisted.allowlisted_domains),
    );

    Ok(json!(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    }))
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
    Ok(serve_tab_result(context, result))
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
    Ok(serve_tab_result(context, result))
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
    Ok(serve_tab_result(context, result))
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
    Ok(serve_tab_result(context, result))
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
    Ok(serve_tab_result(context, result))
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
    Ok(serve_tab_result(context, result))
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
    Ok(serve_tab_result(context, result))
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

fn serve_tab_result(context: ServeTabCommandContext, result: Value) -> Value {
    json!({
        "sessionId": context.session_id,
        "tabId": context.tab_id,
        "result": result,
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
