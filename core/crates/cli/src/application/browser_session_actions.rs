use serde_json::{json, Value};

use super::ports::default_cli_ports;
use crate::*;

pub(crate) fn handle_follow(options: FollowOptions) -> Result<Value, CliError> {
    let ports = default_cli_ports();
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
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
    let source = ports.browser.current_browser_action_source(&persisted)?;
    let target_text = ports
        .browser
        .current_snapshot_ref_text(&persisted.session, &options.target_ref)?;
    let target_href = ports
        .browser
        .current_snapshot_ref_href(&persisted.session, &options.target_ref);
    let target_tag_name = ports
        .browser
        .current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_dom_path_hint = ports
        .browser
        .current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = ports.browser.stable_ref_ordinal_hint(&options.target_ref);
    let result = ports.browser.invoke_follow(PlaywrightFollowParams {
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
    let source_url = ports
        .browser
        .resolved_browser_source_url(&source, &result.final_url);
    let snapshot =
        ports
            .browser
            .compile_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
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
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

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
    let ports = default_cli_ports();
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
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

    let source = ports.browser.current_browser_action_source(&persisted)?;
    let target_text = ports
        .browser
        .current_snapshot_ref_text(&persisted.session, &options.target_ref)?;
    let target_href = ports
        .browser
        .current_snapshot_ref_href(&persisted.session, &options.target_ref);
    let target_tag_name = ports
        .browser
        .current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_dom_path_hint = ports
        .browser
        .current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = ports.browser.stable_ref_ordinal_hint(&options.target_ref);
    let result = ports.browser.invoke_click(PlaywrightClickParams {
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
    let source_url = ports
        .browser
        .resolved_browser_source_url(&source, &result.final_url);
    let snapshot =
        ports
            .browser
            .compile_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    ports
        .browser
        .mark_browser_session_interactive(&mut persisted);
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
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

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
    let ports = default_cli_ports();
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
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

    if ports
        .browser
        .current_snapshot_ref_is_sensitive(&persisted.session, &options.target_ref)
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

    let source = ports.browser.current_browser_action_source(&persisted)?;
    let target_tag_name = ports
        .browser
        .current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_text = if target_tag_name.as_deref() == Some("form") {
        String::new()
    } else {
        ports
            .browser
            .current_snapshot_ref_text(&persisted.session, &options.target_ref)?
    };
    let target_dom_path_hint = ports
        .browser
        .current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = ports.browser.stable_ref_ordinal_hint(&options.target_ref);
    let target_name = ports
        .browser
        .current_snapshot_ref_name(&persisted.session, &options.target_ref);
    let target_input_type = ports
        .browser
        .current_snapshot_ref_input_type(&persisted.session, &options.target_ref);
    let result = ports.browser.invoke_type(PlaywrightTypeParams {
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
    let source_url = ports
        .browser
        .resolved_browser_source_url(&source, &result.final_url);
    let snapshot =
        ports
            .browser
            .compile_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    ports
        .browser
        .mark_browser_session_interactive(&mut persisted);
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: result.final_url.clone(),
        current_html: result.html.clone(),
    });
    if options.sensitive {
        let secret_store_path = ports.session_store.secret_store_path(&options.session_file);
        let mut secrets = ports.session_store.load_secrets(&secret_store_path)?;
        secrets.insert(options.target_ref.clone(), options.value.clone());
        ports
            .session_store
            .save_secrets(&secret_store_path, &secrets)?;
    }
    persisted.browser_trace.push(BrowserActionTraceEntry {
        action: "type".to_string(),
        timestamp: timestamp.clone(),
        target_ref: Some(options.target_ref.clone()),
        direction: None,
        text_value: (!options.sensitive).then_some(options.value.clone()),
        redacted: options.sensitive,
    });
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

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
    let ports = default_cli_ports();
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
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

    let source = ports.browser.current_browser_action_source(&persisted)?;
    let target_tag_name = ports
        .browser
        .current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_text = if target_tag_name.as_deref() == Some("form") {
        String::new()
    } else {
        ports
            .browser
            .current_snapshot_ref_text(&persisted.session, &options.target_ref)?
    };
    let target_dom_path_hint = ports
        .browser
        .current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = ports.browser.stable_ref_ordinal_hint(&options.target_ref);
    let result = ports.browser.invoke_submit(PlaywrightSubmitParams {
        url: source.url.clone(),
        html: source.html.clone(),
        context_dir: source.context_dir.clone(),
        profile_dir: source.profile_dir.clone(),
        target_ref: options.target_ref.clone(),
        target_text,
        target_tag_name,
        target_dom_path_hint,
        target_ordinal_hint,
        prefill: ports.browser.collect_submit_prefill(&persisted, &{
            let mut extra_prefill = options.extra_prefill.clone();
            let secret_store_path = ports.session_store.secret_store_path(&options.session_file);
            let secrets = ports.session_store.load_secrets(&secret_store_path)?;
            extra_prefill.extend(
                secrets
                    .into_iter()
                    .map(|(target_ref, value)| SecretPrefill { target_ref, value }),
            );
            extra_prefill
        }),
        headless: !options.headed,
    })?;
    let source_url = ports
        .browser
        .resolved_browser_source_url(&source, &result.final_url);
    let snapshot =
        ports
            .browser
            .compile_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
    let snapshot = runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source.source_risk,
        source.source_label,
        &timestamp,
    )?;
    ports
        .browser
        .mark_browser_session_interactive(&mut persisted);
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
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

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
    let ports = default_cli_ports();
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_session_block(
        &persisted,
        &kernel,
        ActionName::Paginate,
        "Paginate is blocked because the current snapshot requires review/block.",
        &options.session_file,
    ) {
        return Ok(json!(rejected));
    }
    let source = ports.browser.current_browser_action_source(&persisted)?;
    let current_page = persisted.session.snapshots.len();
    let result = ports.browser.invoke_paginate(PlaywrightPaginateParams {
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
    let source_url = ports
        .browser
        .resolved_browser_source_url(&source, &result.final_url);
    let snapshot =
        ports
            .browser
            .compile_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
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
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

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
    let ports = default_cli_ports();
    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
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
    let source = ports.browser.current_browser_action_source(&persisted)?;
    let target_text = ports
        .browser
        .current_snapshot_ref_text(&persisted.session, &options.target_ref)?;
    let target_tag_name = ports
        .browser
        .current_snapshot_ref_tag_name(&persisted.session, &options.target_ref);
    let target_dom_path_hint = ports
        .browser
        .current_snapshot_ref_dom_path_hint(&persisted.session, &options.target_ref);
    let target_ordinal_hint = ports.browser.stable_ref_ordinal_hint(&options.target_ref);
    let result = ports.browser.invoke_expand(PlaywrightExpandParams {
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
    let source_url = ports
        .browser
        .resolved_browser_source_url(&source, &result.final_url);
    let snapshot =
        ports
            .browser
            .compile_snapshot(&source_url, &result.html, persisted.requested_budget)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
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
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

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
