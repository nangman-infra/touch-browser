use super::{
    context::CliAppContext,
    deps::{
        current_policy_with_allowlist, preflight_interactive_action, preflight_ref_action,
        preflight_session_block, reject_action, succeed_action, ActionFailureKind, ActionName,
        BrowserActionPayload, BrowserActionTraceEntry, CliError, ClickAdapterOutput, ClickOptions,
        ExpandAdapterOutput, ExpandOptions, FollowAdapterOutput, FollowOptions,
        InteractivePreflightOptions, PaginateAdapterOutput, PaginateOptions, PaginationDirection,
        PersistedBrowserState, SecretPrefill, SessionCommandOutput, SubmitAdapterOutput,
        SubmitOptions, TypeAdapterOutput, TypeOptions,
    },
    ports::{
        BrowserClickRequest, BrowserExpandRequest, BrowserFollowRequest, BrowserPaginateRequest,
        BrowserSubmitRequest, BrowserTypeRequest,
    },
};

pub(crate) fn handle_follow(
    ctx: &CliAppContext<'_>,
    options: FollowOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_ref_action(
        &persisted,
        ctx.policy_kernel,
        ActionName::Follow,
        &options.target_ref,
        "Follow target is blocked by the current policy boundary.",
        &options.session_file,
    ) {
        return Ok(rejected);
    }
    let source = ports.browser.current_browser_action_source(&persisted)?;
    let source_risk = source.source_risk.clone();
    let source_label = source.source_label.clone();
    let target = ports
        .browser
        .snapshot_reference(&persisted.session, &options.target_ref)?;
    let result = ports.browser.invoke_follow(BrowserFollowRequest {
        source: source.clone(),
        target,
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
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source_risk,
        source_label,
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
        BrowserActionPayload {
            snapshot,
            adapter: FollowAdapterOutput {
                followed_ref: result.followed_ref,
                target_text: result.target_text,
                target_href: result.target_href,
                clicked_text: result.clicked_text,
                title: result.title,
                visible_text: result.visible_text,
                final_url: result.final_url,
            },
        },
        "Followed a link in the persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    Ok(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_click(
    ctx: &CliAppContext<'_>,
    options: ClickOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_interactive_action(
        &persisted,
        ctx.policy_kernel,
        InteractivePreflightOptions {
            action: ActionName::Click,
            target_ref: Some(&options.target_ref),
            headed: options.headed,
            ack_risks: &options.ack_risks,
            message: "Click target is blocked by the current policy boundary.",
            session_file: &options.session_file,
        },
    ) {
        return Ok(rejected);
    }

    let source = ports.browser.current_browser_action_source(&persisted)?;
    let source_risk = source.source_risk.clone();
    let source_label = source.source_label.clone();
    let target = ports
        .browser
        .snapshot_reference(&persisted.session, &options.target_ref)?;
    let result = ports.browser.invoke_click(BrowserClickRequest {
        source: source.clone(),
        target,
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
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source_risk,
        source_label,
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
        BrowserActionPayload {
            snapshot,
            adapter: ClickAdapterOutput {
                clicked_ref: result.clicked_ref,
                target_text: result.target_text,
                target_href: result.target_href,
                clicked_text: result.clicked_text,
                title: result.title,
                visible_text: result.visible_text,
                final_url: result.final_url,
            },
        },
        "Clicked an interactive target in the persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    Ok(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_type(
    ctx: &CliAppContext<'_>,
    options: TypeOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_interactive_action(
        &persisted,
        ctx.policy_kernel,
        InteractivePreflightOptions {
            action: ActionName::Type,
            target_ref: Some(&options.target_ref),
            headed: options.headed,
            ack_risks: &options.ack_risks,
            message: "Type target is blocked by the current policy boundary.",
            session_file: &options.session_file,
        },
    ) {
        return Ok(rejected);
    }

    let mut target = ports
        .browser
        .snapshot_reference(&persisted.session, &options.target_ref)?;
    if target.sensitive && !options.sensitive {
        let action_result = reject_action(
            ActionName::Type,
            ActionFailureKind::PolicyBlocked,
            "Sensitive credential-like inputs require explicit `--sensitive` opt-in.",
            current_policy_with_allowlist(
                &persisted.session,
                ctx.policy_kernel,
                &persisted.allowlisted_domains,
            ),
        );
        return Ok(SessionCommandOutput {
            action: action_result.clone(),
            result: action_result,
            session_state: persisted.session.state,
            session_file: options.session_file.display().to_string(),
        });
    }

    let source = ports.browser.current_browser_action_source(&persisted)?;
    let source_risk = source.source_risk.clone();
    let source_label = source.source_label.clone();
    if target.tag_name.as_deref() == Some("form") {
        target.text = String::new();
    }
    let result = ports.browser.invoke_type(BrowserTypeRequest {
        source: source.clone(),
        target,
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
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source_risk,
        source_label,
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
        BrowserActionPayload {
            snapshot,
            adapter: TypeAdapterOutput {
                typed_ref: result.typed_ref,
                target_text: result.target_text,
                typed_length: result.typed_length,
                sensitive: options.sensitive,
                title: result.title,
                visible_text: result.visible_text,
                final_url: result.final_url,
            },
        },
        "Typed into an interactive field in the persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    Ok(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_submit(
    ctx: &CliAppContext<'_>,
    options: SubmitOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_interactive_action(
        &persisted,
        ctx.policy_kernel,
        InteractivePreflightOptions {
            action: ActionName::Submit,
            target_ref: Some(&options.target_ref),
            headed: options.headed,
            ack_risks: &options.ack_risks,
            message: "Submit target is blocked by the current policy boundary.",
            session_file: &options.session_file,
        },
    ) {
        return Ok(rejected);
    }

    let source = ports.browser.current_browser_action_source(&persisted)?;
    let source_risk = source.source_risk.clone();
    let source_label = source.source_label.clone();
    let mut target = ports
        .browser
        .snapshot_reference(&persisted.session, &options.target_ref)?;
    if target.tag_name.as_deref() == Some("form") {
        target.text = String::new();
    }
    let result = ports.browser.invoke_submit(BrowserSubmitRequest {
        source: source.clone(),
        target,
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
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source_risk,
        source_label,
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
        BrowserActionPayload {
            snapshot,
            adapter: SubmitAdapterOutput {
                submitted_ref: result.submitted_ref,
                target_text: result.target_text,
                title: result.title,
                visible_text: result.visible_text,
                final_url: result.final_url,
            },
        },
        "Submitted an interactive form or submit control in the persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    Ok(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_paginate(
    ctx: &CliAppContext<'_>,
    options: PaginateOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_session_block(
        &persisted,
        ctx.policy_kernel,
        ActionName::Paginate,
        "Paginate is blocked because the current snapshot requires review/block.",
        &options.session_file,
    ) {
        return Ok(rejected);
    }
    let source = ports.browser.current_browser_action_source(&persisted)?;
    let source_risk = source.source_risk.clone();
    let source_label = source.source_label.clone();
    let current_page = persisted.session.snapshots.len();
    let result = ports.browser.invoke_paginate(BrowserPaginateRequest {
        source: source.clone(),
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
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source_risk,
        source_label,
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
        BrowserActionPayload {
            snapshot,
            adapter: PaginateAdapterOutput {
                page: result.page,
                clicked_text: result.clicked_text,
                title: result.title,
                visible_text: result.visible_text,
                final_url: result.final_url,
            },
        },
        "Paginated persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    Ok(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_expand(
    ctx: &CliAppContext<'_>,
    options: ExpandOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    if let Some(rejected) = preflight_ref_action(
        &persisted,
        ctx.policy_kernel,
        ActionName::Expand,
        &options.target_ref,
        "Expand target is blocked by the current policy boundary.",
        &options.session_file,
    ) {
        return Ok(rejected);
    }
    let source = ports.browser.current_browser_action_source(&persisted)?;
    let source_risk = source.source_risk.clone();
    let source_label = source.source_label.clone();
    let target = ports
        .browser
        .snapshot_reference(&persisted.session, &options.target_ref)?;
    let result = ports.browser.invoke_expand(BrowserExpandRequest {
        source: source.clone(),
        target,
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
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &source_url,
        snapshot,
        source_risk,
        source_label,
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
        BrowserActionPayload {
            snapshot,
            adapter: ExpandAdapterOutput {
                expanded_ref: result.expanded_ref,
                target_text: result.target_text,
                clicked_text: result.clicked_text,
                title: result.title,
                visible_text: result.visible_text,
                final_url: result.final_url,
            },
        },
        "Expanded persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    Ok(SessionCommandOutput {
        action: action_result.clone(),
        result: action_result,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}
