use super::{
    context::CliAppContext,
    deps::{
        approved_risk_labels, checkpoint_approval_panel, checkpoint_candidates,
        checkpoint_playbook, checkpoint_provider_hints, current_policy_with_allowlist,
        current_timestamp, is_fixture_target, is_search_results_target, policy_profile_label,
        promoted_policy_profile_for_risks, recommend_requested_tokens, recommended_policy_profile,
        required_ack_risks, resolve_latest_search_session_file, succeed_action,
        verify_action_result_if_requested, ActionName, ApproveOptions, BrowserReplayCommandOutput,
        ClaimInput, CliError, ClickOptions, CompactSnapshotOutput, ExpandOptions, FollowOptions,
        OutputFormat, PaginateOptions, PaginationDirection, PersistedBrowserState, ReadViewOutput,
        RuntimeError, SessionApprovalCommandOutput, SessionApprovalValue,
        SessionCheckpointCommandOutput, SessionCheckpointReport, SessionCloseCommandOutput,
        SessionCloseResultValue, SessionCommandOutput, SessionExtractCommandOutput,
        SessionExtractOptions, SessionFileOptions, SessionPolicyCommandOutput,
        SessionProfileCommandOutput, SessionProfileSetOptions, SessionProfileValue,
        SessionReadOptions, SessionRefreshOptions, SessionSynthesisCommandOutput,
        SessionSynthesizeOptions, SourceRisk, SubmitOptions, TargetOptions,
        TelemetryRecentCommandOutput, TelemetryRecentOptions, TelemetrySummaryCommandOutput,
        TypeOptions,
    },
    ports::BrowserSnapshotCaptureRequest,
    presentation_support::{render_compact_snapshot, render_session_synthesis_markdown},
};

use std::{fs, path::PathBuf};

fn claim_inputs_from_statements(statements: &[String]) -> Result<Vec<ClaimInput>, CliError> {
    let claims = statements
        .iter()
        .map(|statement| statement.trim())
        .filter(|statement| !statement.is_empty())
        .enumerate()
        .map(|(index, statement)| ClaimInput {
            id: format!("c{}", index + 1),
            statement: statement.to_string(),
        })
        .collect::<Vec<_>>();
    if claims.is_empty() {
        return Err(CliError::Usage(
            "session-extract requires at least one non-empty `--claim` statement.".to_string(),
        ));
    }
    Ok(claims)
}

pub(crate) fn handle_session_snapshot(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let persisted = ports.session_store.load_session(&options.session_file)?;
    let snapshot = persisted
        .session
        .current_snapshot_record()
        .ok_or(RuntimeError::NoCurrentSnapshot)?
        .snapshot
        .clone();
    let action_result = succeed_action(
        ActionName::Read,
        "snapshot-document",
        snapshot,
        "Read persisted browser-backed snapshot.",
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

pub(crate) fn handle_session_compact(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<CompactSnapshotOutput, CliError> {
    let persisted = ctx
        .ports
        .session_store
        .load_session(&options.session_file)?;
    let snapshot = persisted
        .session
        .current_snapshot_record()
        .ok_or(RuntimeError::NoCurrentSnapshot)?
        .snapshot
        .clone();

    Ok(CompactSnapshotOutput::new(
        &snapshot,
        Some(persisted.session.state),
        Some(options.session_file.display().to_string()),
    ))
}

pub(crate) fn handle_session_read(
    ctx: &CliAppContext<'_>,
    options: SessionReadOptions,
) -> Result<ReadViewOutput, CliError> {
    let persisted = ctx
        .ports
        .session_store
        .load_session(&options.session_file)?;
    let snapshot = persisted
        .session
        .current_snapshot_record()
        .ok_or(RuntimeError::NoCurrentSnapshot)?
        .snapshot
        .clone();

    Ok(ReadViewOutput::new(
        &snapshot,
        Some(persisted.session.state),
        Some(options.session_file.display().to_string()),
        options.main_only,
    ))
}

pub(crate) fn handle_session_refresh(
    ctx: &CliAppContext<'_>,
    options: SessionRefreshOptions,
) -> Result<SessionCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    let current_search_identity = persisted
        .session
        .current_snapshot_record()
        .map(|record| is_search_results_target(&record.snapshot.source.source_url))
        .unwrap_or(false);
    let primary_capture = ports
        .browser
        .invoke_snapshot(BrowserSnapshotCaptureRequest {
            url: None,
            html: None,
            context_dir: persisted.browser_context_dir.clone(),
            profile_dir: persisted.browser_profile_dir.clone(),
            budget: persisted.requested_budget,
            headless: !options.headed,
            search_identity: current_search_identity,
        })?;
    let (capture, effective_budget, snapshot) = match ports.browser.compile_snapshot(
        &primary_capture.final_url,
        &primary_capture.html,
        recommend_requested_tokens(&primary_capture.html, persisted.requested_budget),
    ) {
        Ok(snapshot) => {
            let effective_budget =
                recommend_requested_tokens(&primary_capture.html, persisted.requested_budget);
            (primary_capture, effective_budget, snapshot)
        }
        Err(_) => {
            let source = ports.browser.current_browser_action_source(&persisted)?;
            let fallback_capture =
                ports
                    .browser
                    .invoke_snapshot(BrowserSnapshotCaptureRequest {
                        url: source.url,
                        html: source.html,
                        context_dir: source.context_dir,
                        profile_dir: source.profile_dir,
                        budget: persisted.requested_budget,
                        headless: !options.headed,
                        search_identity: is_search_results_target(&source.source_url),
                    })?;
            let effective_budget =
                recommend_requested_tokens(&fallback_capture.html, persisted.requested_budget);
            let snapshot = ports.browser.compile_snapshot(
                &fallback_capture.final_url,
                &fallback_capture.html,
                effective_budget,
            )?;
            (fallback_capture, effective_budget, snapshot)
        }
    };
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
    let source_risk = persisted
        .session
        .current_snapshot_record()
        .map(|record| record.source_risk.clone())
        .unwrap_or(SourceRisk::Low);
    let source_label = persisted
        .session
        .current_snapshot_record()
        .and_then(|record| record.source_label.clone());
    let snapshot = ctx.runtime.open_snapshot(
        &mut persisted.session,
        &capture.final_url,
        snapshot,
        source_risk,
        source_label,
        &timestamp,
    )?;
    persisted.requested_budget = effective_budget;
    persisted.browser_state = Some(PersistedBrowserState {
        current_url: capture.final_url.clone(),
        current_html: capture.html.clone(),
    });
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

    let action_result = succeed_action(
        ActionName::Read,
        "snapshot-document",
        snapshot,
        "Refreshed the persisted browser session from the current live page state.",
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

pub(crate) fn handle_session_extract(
    ctx: &CliAppContext<'_>,
    options: SessionExtractOptions,
) -> Result<SessionExtractCommandOutput, CliError> {
    let ports = ctx.ports;
    let session_file =
        resolve_latest_search_session_file(options.session_file.as_ref(), options.engine)?;
    let mut persisted = ports.session_store.load_session(&session_file)?;
    let timestamp = ports.browser.next_session_timestamp(&persisted.session);
    let claims = claim_inputs_from_statements(&options.claims)?;
    let report = ctx
        .runtime
        .extract(&mut persisted.session, claims.clone(), &timestamp)?;
    let extract_result = succeed_action(
        ActionName::Extract,
        "evidence-report",
        report,
        "Extracted evidence report from persisted browser session.",
        current_policy_with_allowlist(
            &persisted.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    );
    let extract_result = verify_action_result_if_requested(
        ports.verifier,
        extract_result?,
        &mut persisted.session,
        &claims,
        options.verifier_command.as_deref(),
        &timestamp,
    )?;
    ports
        .session_store
        .save_session(&session_file, &persisted)?;

    Ok(SessionExtractCommandOutput {
        extract: extract_result.clone(),
        result: extract_result,
        session_state: persisted.session.state,
        session_file: session_file.display().to_string(),
    })
}

pub(crate) fn handle_session_policy(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<SessionPolicyCommandOutput, CliError> {
    let ports = ctx.ports;
    let persisted = ports.session_store.load_session(&options.session_file)?;
    let report = current_policy_with_allowlist(
        &persisted.session,
        ctx.policy_kernel,
        &persisted.allowlisted_domains,
    )
    .ok_or_else(|| CliError::Usage("Policy command requires an open snapshot.".to_string()))?;

    Ok(SessionPolicyCommandOutput {
        policy: report.clone(),
        result: report,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_session_profile(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<SessionProfileCommandOutput, CliError> {
    let persisted = ctx
        .ports
        .session_store
        .load_session(&options.session_file)?;
    let policy_profile = policy_profile_label(persisted.session.state.policy_profile).to_string();
    Ok(SessionProfileCommandOutput {
        policy_profile: policy_profile.clone(),
        result: SessionProfileValue { policy_profile },
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_set_profile(
    ctx: &CliAppContext<'_>,
    options: SessionProfileSetOptions,
) -> Result<SessionProfileCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    persisted.session.state.policy_profile = options.profile;
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

    let policy_profile = policy_profile_label(persisted.session.state.policy_profile).to_string();
    Ok(SessionProfileCommandOutput {
        policy_profile: policy_profile.clone(),
        result: SessionProfileValue { policy_profile },
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_session_checkpoint(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<SessionCheckpointCommandOutput, CliError> {
    let ports = ctx.ports;
    let persisted = ports.session_store.load_session(&options.session_file)?;
    let record = persisted
        .session
        .current_snapshot_record()
        .ok_or_else(|| CliError::Usage("checkpoint requires an open snapshot.".to_string()))?;
    let policy = current_policy_with_allowlist(
        &persisted.session,
        ctx.policy_kernel,
        &persisted.allowlisted_domains,
    )
    .ok_or_else(|| CliError::Usage("checkpoint requires an open snapshot.".to_string()))?;
    let provider_hints = checkpoint_provider_hints(&record.snapshot, &policy);
    let required_ack_risks = required_ack_risks(&policy);
    let approved_risks = approved_risk_labels(&persisted.approved_risks);
    let recommended_profile = recommended_policy_profile(&policy);
    let checkpoint = SessionCheckpointReport {
        provider_hints: provider_hints.clone(),
        required_ack_risks: required_ack_risks.clone(),
        approved_risks: approved_risks.clone(),
        active_policy_profile: policy_profile_label(persisted.session.state.policy_profile)
            .to_string(),
        recommended_policy_profile: policy_profile_label(recommended_profile).to_string(),
        approval_panel: checkpoint_approval_panel(
            &provider_hints,
            &required_ack_risks,
            &approved_risks,
            persisted.session.state.policy_profile,
            recommended_profile,
            &policy,
        ),
        playbook: checkpoint_playbook(
            &provider_hints,
            &required_ack_risks,
            &approved_risks,
            &record.snapshot,
            recommended_profile,
        ),
        candidates: checkpoint_candidates(&record.snapshot),
        requires_headed_supervision: !is_fixture_target(&record.snapshot.source.source_url),
        source_url: record.snapshot.source.source_url.clone(),
        source_title: record.snapshot.source.title.clone(),
    };

    Ok(SessionCheckpointCommandOutput {
        checkpoint: checkpoint.clone(),
        result: checkpoint,
        policy,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_session_synthesize(
    ctx: &CliAppContext<'_>,
    options: SessionSynthesizeOptions,
) -> Result<SessionSynthesisCommandOutput, CliError> {
    let persisted = ctx
        .ports
        .session_store
        .load_session(&options.session_file)?;
    let report = ctx.runtime.synthesize_session(
        &persisted.session,
        &current_timestamp(),
        options.note_limit,
    )?;
    let markdown = (options.format == OutputFormat::Markdown)
        .then(|| render_session_synthesis_markdown(&report));

    Ok(SessionSynthesisCommandOutput {
        report: report.clone(),
        result: report,
        format: options.format,
        markdown,
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_approve(
    ctx: &CliAppContext<'_>,
    options: ApproveOptions,
) -> Result<SessionApprovalCommandOutput, CliError> {
    let ports = ctx.ports;
    let mut persisted = ports.session_store.load_session(&options.session_file)?;
    for ack_risk in &options.ack_risks {
        persisted.approved_risks.insert(*ack_risk);
    }
    persisted.session.state.policy_profile = promoted_policy_profile_for_risks(
        persisted.session.state.policy_profile,
        &persisted.approved_risks,
    );
    ports
        .session_store
        .save_session(&options.session_file, &persisted)?;

    let approved_risks = approved_risk_labels(&persisted.approved_risks);
    let policy_profile = policy_profile_label(persisted.session.state.policy_profile).to_string();
    Ok(SessionApprovalCommandOutput {
        approved_risks: approved_risks.clone(),
        policy_profile: policy_profile.clone(),
        result: SessionApprovalValue {
            approved_risks,
            policy_profile,
        },
        session_state: persisted.session.state,
        session_file: options.session_file.display().to_string(),
    })
}

pub(crate) fn handle_telemetry_summary(
    ctx: &CliAppContext<'_>,
) -> Result<TelemetrySummaryCommandOutput, CliError> {
    let summary = ctx.ports.telemetry.summary()?;
    Ok(TelemetrySummaryCommandOutput {
        summary: summary.clone(),
        result: summary,
    })
}

pub(crate) fn handle_telemetry_recent(
    ctx: &CliAppContext<'_>,
    options: TelemetryRecentOptions,
) -> Result<TelemetryRecentCommandOutput, CliError> {
    let events = ctx.ports.telemetry.recent_events(options.limit)?;
    Ok(TelemetryRecentCommandOutput {
        limit: options.limit,
        events: events.clone(),
        result: events,
    })
}

pub(crate) fn handle_session_close(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<SessionCloseCommandOutput, CliError> {
    let ports = ctx.ports;
    let persisted = if options.session_file.exists() {
        Some(ports.session_store.load_session(&options.session_file)?)
    } else {
        None
    };
    let removed = if options.session_file.exists() {
        fs::remove_file(&options.session_file)?;
        true
    } else {
        false
    };
    let secret_store_path = ports.session_store.secret_store_path(&options.session_file);
    if secret_store_path.exists() {
        fs::remove_file(secret_store_path)?;
    }

    if let Some(persisted) = persisted {
        if persisted.browser_profile_dir.is_none() {
            if let Some(context_dir) = persisted.browser_context_dir {
                let context_path = PathBuf::from(context_dir);
                if context_path.exists() {
                    fs::remove_dir_all(context_path)?;
                }
            }
        }
    }

    Ok(SessionCloseCommandOutput {
        session_file: options.session_file.display().to_string(),
        removed,
        result: SessionCloseResultValue {
            session_file: options.session_file.display().to_string(),
            removed,
        },
    })
}

pub(crate) fn handle_browser_replay(
    ctx: &CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<BrowserReplayCommandOutput, CliError> {
    let ports = ctx.ports;
    let persisted = ports.session_store.load_session(&options.session_file)?;
    let source_secrets = ports
        .session_store
        .load_secrets(&ports.session_store.secret_store_path(&options.session_file))?;
    let origin = persisted.browser_origin.clone().ok_or_else(|| {
        CliError::Usage("browser-replay requires a session created by browser open.".to_string())
    })?;
    let replay_session_file = std::env::temp_dir().join(format!(
        "touch-browser-browser-replay-{}.json",
        std::process::id()
    ));

    super::research_commands::handle_open(
        ctx,
        TargetOptions {
            target: origin.target,
            budget: persisted.requested_budget,
            source_risk: origin.source_risk,
            source_label: origin.source_label,
            allowlisted_domains: persisted.allowlisted_domains.clone(),
            browser: true,
            headed: !persisted.headless,
            main_only: false,
            session_file: Some(replay_session_file.clone()),
        },
    )?;

    for entry in &persisted.browser_trace {
        match entry.action.as_str() {
            "follow" => {
                let target_ref = entry.target_ref.clone().ok_or_else(|| {
                    CliError::Usage(
                        "browser replay follow entry is missing a target ref.".to_string(),
                    )
                })?;
                super::browser_session_actions::handle_follow(
                    ctx,
                    FollowOptions {
                        session_file: replay_session_file.clone(),
                        target_ref,
                        headed: !persisted.headless,
                    },
                )?;
            }
            "click" => {
                let target_ref = entry.target_ref.clone().ok_or_else(|| {
                    CliError::Usage(
                        "browser replay click entry is missing a target ref.".to_string(),
                    )
                })?;
                super::browser_session_actions::handle_click(
                    ctx,
                    ClickOptions {
                        session_file: replay_session_file.clone(),
                        target_ref,
                        headed: !persisted.headless,
                        ack_risks: Vec::new(),
                    },
                )?;
            }
            "type" => {
                let target_ref = entry.target_ref.clone().ok_or_else(|| {
                    CliError::Usage(
                        "browser replay type entry is missing a target ref.".to_string(),
                    )
                })?;
                let (value, sensitive) = if entry.redacted {
                    (
                        source_secrets.get(&target_ref).cloned().ok_or_else(|| {
                            CliError::Usage(
                                "browser replay cannot restore a redacted sensitive type action without a stored secret sidecar."
                                    .to_string(),
                            )
                        })?,
                        true,
                    )
                } else {
                    (
                        entry.text_value.clone().ok_or_else(|| {
                            CliError::Usage(
                                "browser replay type entry is missing a non-sensitive value."
                                    .to_string(),
                            )
                        })?,
                        false,
                    )
                };
                super::browser_session_actions::handle_type(
                    ctx,
                    TypeOptions {
                        session_file: replay_session_file.clone(),
                        target_ref,
                        value,
                        headed: !persisted.headless,
                        sensitive,
                        ack_risks: Vec::new(),
                    },
                )?;
            }
            "submit" => {
                let target_ref = entry.target_ref.clone().ok_or_else(|| {
                    CliError::Usage(
                        "browser replay submit entry is missing a target ref.".to_string(),
                    )
                })?;
                super::browser_session_actions::handle_submit(
                    ctx,
                    SubmitOptions {
                        session_file: replay_session_file.clone(),
                        target_ref,
                        headed: !persisted.headless,
                        ack_risks: Vec::new(),
                        extra_prefill: Vec::new(),
                    },
                )?;
            }
            "paginate" => {
                let direction = match entry.direction.as_deref() {
                    Some("next") => PaginationDirection::Next,
                    Some("prev") => PaginationDirection::Prev,
                    _ => {
                        return Err(CliError::Usage(
                            "browser replay paginate entry is missing a valid direction."
                                .to_string(),
                        ))
                    }
                };
                super::browser_session_actions::handle_paginate(
                    ctx,
                    PaginateOptions {
                        session_file: replay_session_file.clone(),
                        direction,
                        headed: !persisted.headless,
                    },
                )?;
            }
            "expand" => {
                let target_ref = entry.target_ref.clone().ok_or_else(|| {
                    CliError::Usage(
                        "browser replay expand entry is missing a target ref.".to_string(),
                    )
                })?;
                super::browser_session_actions::handle_expand(
                    ctx,
                    ExpandOptions {
                        session_file: replay_session_file.clone(),
                        target_ref,
                        headed: !persisted.headless,
                    },
                )?;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "browser replay does not support action `{other}`."
                )));
            }
        }
    }

    let replayed = ports.session_store.load_session(&replay_session_file)?;
    let compact = replayed
        .session
        .current_snapshot_record()
        .map(|record| render_compact_snapshot(&record.snapshot))
        .unwrap_or_default();
    let session_state = replayed.session.state.clone();
    handle_session_close(
        ctx,
        SessionFileOptions {
            session_file: replay_session_file,
        },
    )?;

    Ok(BrowserReplayCommandOutput {
        replayed_actions: persisted.browser_trace.len(),
        compact_text: compact,
        session_state,
    })
}
