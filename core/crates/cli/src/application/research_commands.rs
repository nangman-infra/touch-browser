use crate::*;

use serde_json::json;

pub(crate) fn handle_search(options: SearchOptions) -> Result<Value, CliError> {
    let search_url = build_search_url(options.engine, &options.query)?;
    let session_file = resolve_search_session_file(options.session_file.as_ref(), options.engine);
    let browser_profile_dir = options
        .profile_dir
        .as_ref()
        .map(|path| path.display().to_string());
    let browser_context_dir = if browser_profile_dir.is_some() {
        None
    } else {
        Some(
            browser_context_dir_for_session_file(&session_file)
                .display()
                .to_string(),
        )
    };
    let context = open_browser_session(
        &search_url,
        options.budget,
        Some(SourceRisk::Low),
        Some(search_engine_source_label(options.engine).to_string()),
        options.headed,
        browser_context_dir.clone(),
        browser_profile_dir.clone(),
        "sclisearch001",
        DEFAULT_OPENED_AT,
    )?;
    let report = build_search_report(
        options.engine,
        &options.query,
        &search_url,
        &context.snapshot,
        &context.browser_state.current_html,
        &context.browser_state.current_url,
        DEFAULT_OPENED_AT,
    );

    save_browser_cli_session(
        &session_file,
        &build_browser_cli_session(
            &context.session,
            context.snapshot.budget.requested_tokens,
            !options.headed,
            Some(context.browser_state.clone()),
            context.browser_context_dir.clone(),
            context.browser_profile_dir.clone(),
            Some(BrowserOrigin {
                target: search_url.clone(),
                source_risk: Some(SourceRisk::Low),
                source_label: Some(search_engine_source_label(options.engine).to_string()),
            }),
            Vec::new(),
            Vec::new(),
            Some(report.clone()),
        ),
    )?;

    Ok(json!({
        "query": options.query,
        "engine": options.engine,
        "searchUrl": search_url,
        "resultCount": report.result_count,
        "search": report.clone(),
        "result": report,
        "browserContextDir": browser_context_dir,
        "browserProfileDir": browser_profile_dir,
        "sessionState": context.session.state,
        "sessionFile": session_file.display().to_string(),
    }))
}

pub(crate) fn handle_search_open_result(
    options: SearchOpenResultOptions,
) -> Result<Value, CliError> {
    let session_file = resolve_search_session_file(options.session_file.as_ref(), options.engine);
    let persisted = load_browser_cli_session(&session_file)?;
    let latest_search = persisted.latest_search.clone().ok_or_else(|| {
        CliError::Usage(
            "This browser session does not contain saved search results. Run `touch-browser search ... --session-file <path>` first.".to_string(),
        )
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
        .find(|result| result.rank == options.rank)
        .cloned()
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Saved search results do not contain rank {}.",
                options.rank
            ))
        })?;

    let context = open_browser_session(
        &selected.url,
        persisted.requested_budget,
        Some(SourceRisk::Low),
        None,
        options.headed,
        persisted.browser_context_dir.clone(),
        persisted.browser_profile_dir.clone(),
        "scliopen001",
        DEFAULT_OPENED_AT,
    )?;
    save_browser_cli_session(
        &session_file,
        &build_browser_cli_session(
            &context.session,
            context.snapshot.budget.requested_tokens,
            !options.headed,
            Some(context.browser_state.clone()),
            context.browser_context_dir.clone(),
            context.browser_profile_dir.clone(),
            Some(BrowserOrigin {
                target: selected.url.clone(),
                source_risk: Some(SourceRisk::Low),
                source_label: None,
            }),
            persisted.allowlisted_domains.clone(),
            Vec::new(),
            Some(latest_search.clone()),
        ),
    )?;
    let opened = json!(succeed_action(
        ActionName::Open,
        "snapshot-document",
        json!(context.snapshot),
        "Opened browser-backed document.",
        current_policy_with_allowlist(
            &context.session,
            &PolicyKernel,
            &persisted.allowlisted_domains
        ),
    ));

    Ok(json!({
        "selectedResult": selected,
        "result": opened,
        "sessionFile": session_file.display().to_string(),
    }))
}

pub(crate) fn handle_search_open_top(options: SearchOpenTopOptions) -> Result<Value, CliError> {
    let search_session_file =
        resolve_search_session_file(options.session_file.as_ref(), options.engine);
    let persisted = load_browser_cli_session(&search_session_file)?;
    let latest_search = persisted.latest_search.clone().ok_or_else(|| {
        CliError::Usage(
            "This browser session does not contain saved search results. Run `touch-browser search ... --session-file <path>` first.".to_string(),
        )
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
            .take(options.limit)
            .collect::<Vec<_>>()
    } else {
        latest_search
            .recommended_result_ranks
            .iter()
            .copied()
            .take(options.limit)
            .collect::<Vec<_>>()
    };

    let opened = selected_ranks
        .into_iter()
        .filter_map(|rank| {
            latest_search
                .results
                .iter()
                .find(|result| result.rank == rank)
                .cloned()
        })
        .map(|selected| {
            let result_session_file =
                derived_search_result_session_file(&search_session_file, selected.rank);
            let context = open_browser_session(
                &selected.url,
                persisted.requested_budget,
                Some(SourceRisk::Low),
                None,
                options.headed,
                persisted.browser_context_dir.clone(),
                persisted.browser_profile_dir.clone(),
                "scliopen001",
                DEFAULT_OPENED_AT,
            )?;
            save_browser_cli_session(
                &result_session_file,
                &build_browser_cli_session(
                    &context.session,
                    context.snapshot.budget.requested_tokens,
                    !options.headed,
                    Some(context.browser_state.clone()),
                    context.browser_context_dir.clone(),
                    context.browser_profile_dir.clone(),
                    Some(BrowserOrigin {
                        target: selected.url.clone(),
                        source_risk: Some(SourceRisk::Low),
                        source_label: None,
                    }),
                    persisted.allowlisted_domains.clone(),
                    Vec::new(),
                    None,
                ),
            )?;
            let opened = json!(succeed_action(
                ActionName::Open,
                "snapshot-document",
                json!(context.snapshot),
                "Opened browser-backed document.",
                current_policy_with_allowlist(
                    &context.session,
                    &PolicyKernel,
                    &persisted.allowlisted_domains,
                ),
            ));

            Ok::<Value, CliError>(json!({
                "rank": selected.rank,
                "selectedResult": selected,
                "sessionFile": result_session_file.display().to_string(),
                "result": opened,
            }))
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "searchSessionFile": search_session_file.display().to_string(),
        "openedCount": opened.len(),
        "opened": opened,
    }))
}

pub(crate) fn handle_open(options: TargetOptions) -> Result<Value, CliError> {
    if options.session_file.is_some() && !options.browser {
        return Err(CliError::Usage(
            "`--session-file` is currently supported only with `--browser`.".to_string(),
        ));
    }

    if options.browser {
        return handle_browser_open(options);
    }

    if is_fixture_target(&options.target) {
        let catalog = load_fixture_catalog()?;
        let runtime = ReadOnlyRuntime::default();
        let vm = ReadOnlyActionVm::default();
        let mut session = runtime.start_session("scliopen001", DEFAULT_OPENED_AT);
        let result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(options.target),
                risk_class: RiskClass::Low,
                reason: "Open a fixture-backed semantic snapshot.".to_string(),
                input: None,
            },
            DEFAULT_OPENED_AT,
        );
        return Ok(json!(result));
    }

    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut acquisition = AcquisitionEngine::new(AcquisitionConfig::default())?;
    let mut session = runtime.start_session("scliopen001", DEFAULT_OPENED_AT);
    let source_risk = options.source_risk.unwrap_or(SourceRisk::Low);
    let snapshot = runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        source_risk.clone(),
        options.source_label,
        DEFAULT_OPENED_AT,
    )?;
    let policy = current_policy_with_allowlist(&session, &kernel, &options.allowlisted_domains);

    Ok(json!(succeed_action(
        ActionName::Open,
        "snapshot-document",
        json!(snapshot),
        "Opened live document.",
        policy,
    )))
}

pub(crate) fn handle_browser_open(options: TargetOptions) -> Result<Value, CliError> {
    let kernel = PolicyKernel;
    let browser_context_dir = options
        .session_file
        .as_ref()
        .map(|path| browser_context_dir_for_session_file(path.as_path()))
        .map(|path| path.display().to_string());
    let context = open_browser_session(
        &options.target,
        options.budget,
        options.source_risk.clone(),
        options.source_label.clone(),
        options.headed,
        browser_context_dir.clone(),
        None,
        "scliopen001",
        DEFAULT_OPENED_AT,
    )?;
    if let Some(session_file) = options.session_file.as_ref() {
        save_browser_cli_session(
            session_file,
            &build_browser_cli_session(
                &context.session,
                context.snapshot.budget.requested_tokens,
                !options.headed,
                Some(context.browser_state.clone()),
                context.browser_context_dir.clone(),
                context.browser_profile_dir.clone(),
                Some(BrowserOrigin {
                    target: options.target.clone(),
                    source_risk: options.source_risk,
                    source_label: options.source_label.clone(),
                }),
                options.allowlisted_domains.clone(),
                Vec::new(),
                None,
            ),
        )?;
    }

    Ok(json!(succeed_action(
        ActionName::Open,
        "snapshot-document",
        json!(context.snapshot),
        "Opened browser-backed document.",
        current_policy_with_allowlist(&context.session, &kernel, &options.allowlisted_domains,),
    )))
}

pub(crate) fn handle_compact_view(options: TargetOptions) -> Result<Value, CliError> {
    if options.browser {
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| browser_context_dir_for_session_file(path.as_path()))
            .map(|path| path.display().to_string());
        let context = open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir.clone(),
            None,
            "sclicompact001",
            DEFAULT_OPENED_AT,
        )?;

        if let Some(session_file) = options.session_file.as_ref() {
            save_browser_cli_session(
                session_file,
                &build_browser_cli_session(
                    &context.session,
                    context.snapshot.budget.requested_tokens,
                    !options.headed,
                    Some(context.browser_state.clone()),
                    context.browser_context_dir.clone(),
                    context.browser_profile_dir.clone(),
                    Some(BrowserOrigin {
                        target: options.target.clone(),
                        source_risk: options.source_risk,
                        source_label: options.source_label.clone(),
                    }),
                    options.allowlisted_domains.clone(),
                    Vec::new(),
                    None,
                ),
            )?;
        }

        return Ok(json!(CompactSnapshotOutput::new(
            &context.snapshot,
            Some(context.session.state),
            options.session_file.map(|path| path.display().to_string()),
        )));
    }

    if is_fixture_target(&options.target) {
        let catalog = load_fixture_catalog()?;
        let runtime = ReadOnlyRuntime::default();
        let mut session = runtime.start_session("sclicompact001", DEFAULT_OPENED_AT);
        let snapshot = runtime.open(&mut session, &catalog, &options.target, DEFAULT_OPENED_AT)?;
        return Ok(json!(CompactSnapshotOutput::new(
            &snapshot,
            Some(session.state),
            None,
        )));
    }

    let runtime = ReadOnlyRuntime::default();
    let mut acquisition = AcquisitionEngine::new(AcquisitionConfig::default())?;
    let mut session = runtime.start_session("sclicompact001", DEFAULT_OPENED_AT);
    let snapshot = runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.unwrap_or(SourceRisk::Low),
        options.source_label,
        DEFAULT_OPENED_AT,
    )?;

    Ok(json!(CompactSnapshotOutput::new(
        &snapshot,
        Some(session.state),
        None,
    )))
}

pub(crate) fn handle_read_view(options: TargetOptions) -> Result<Value, CliError> {
    if options.browser {
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| browser_context_dir_for_session_file(path.as_path()))
            .map(|path| path.display().to_string());
        let context = open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir.clone(),
            None,
            "scliread001",
            DEFAULT_OPENED_AT,
        )?;

        if let Some(session_file) = options.session_file.as_ref() {
            save_browser_cli_session(
                session_file,
                &build_browser_cli_session(
                    &context.session,
                    context.snapshot.budget.requested_tokens,
                    !options.headed,
                    Some(context.browser_state.clone()),
                    context.browser_context_dir.clone(),
                    context.browser_profile_dir.clone(),
                    Some(BrowserOrigin {
                        target: options.target.clone(),
                        source_risk: options.source_risk,
                        source_label: options.source_label.clone(),
                    }),
                    options.allowlisted_domains.clone(),
                    Vec::new(),
                    None,
                ),
            )?;
        }

        return Ok(json!(ReadViewOutput::new(
            &context.snapshot,
            Some(context.session.state),
            options.session_file.map(|path| path.display().to_string()),
            options.main_only,
        )));
    }

    if is_fixture_target(&options.target) {
        let catalog = load_fixture_catalog()?;
        let runtime = ReadOnlyRuntime::default();
        let mut session = runtime.start_session("scliread001", DEFAULT_OPENED_AT);
        let snapshot = runtime.open(&mut session, &catalog, &options.target, DEFAULT_OPENED_AT)?;
        return Ok(json!(ReadViewOutput::new(
            &snapshot,
            Some(session.state),
            None,
            options.main_only,
        )));
    }

    let runtime = ReadOnlyRuntime::default();
    let mut acquisition = AcquisitionEngine::new(AcquisitionConfig::default())?;
    let mut session = runtime.start_session("scliread001", DEFAULT_OPENED_AT);
    let snapshot = runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.unwrap_or(SourceRisk::Low),
        options.source_label,
        DEFAULT_OPENED_AT,
    )?;

    Ok(json!(ReadViewOutput::new(
        &snapshot,
        Some(session.state),
        None,
        options.main_only,
    )))
}

pub(crate) fn handle_extract(options: ExtractOptions) -> Result<Value, CliError> {
    if options.session_file.is_some() && !options.browser {
        return Err(CliError::Usage(
            "`--session-file` is currently supported only with `--browser` on target commands. Use `session-extract` for persisted sessions.".to_string(),
        ));
    }

    let claims = options
        .claims
        .iter()
        .enumerate()
        .map(|(index, statement)| ClaimInput {
            id: format!("c{}", index + 1),
            statement: statement.clone(),
        })
        .collect::<Vec<_>>();

    if options.browser {
        let kernel = PolicyKernel;
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| browser_context_dir_for_session_file(path.as_path()))
            .map(|path| path.display().to_string());
        let context = open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir.clone(),
            None,
            "scliextract001",
            DEFAULT_OPENED_AT,
        )?;
        let open_result = succeed_action(
            ActionName::Open,
            "snapshot-document",
            json!(context.snapshot),
            "Opened browser-backed document.",
            current_policy_with_allowlist(&context.session, &kernel, &options.allowlisted_domains),
        );
        let mut session = context.session;
        let extract_timestamp = slot_timestamp(1, 30);
        let report = context
            .runtime
            .extract(&mut session, claims.clone(), &extract_timestamp)?;
        let extract_result = succeed_action(
            ActionName::Extract,
            "evidence-report",
            json!(report),
            "Extracted evidence report from browser-backed snapshot.",
            current_policy_with_allowlist(&session, &kernel, &options.allowlisted_domains),
        );
        let extract_result = verify_action_result_if_requested(
            extract_result,
            &mut session,
            &claims,
            options.verifier_command.as_deref(),
            &extract_timestamp,
        )?;
        if let Some(session_file) = options.session_file.as_ref() {
            save_browser_cli_session(
                session_file,
                &build_browser_cli_session(
                    &session,
                    context.snapshot.budget.requested_tokens,
                    !options.headed,
                    Some(context.browser_state.clone()),
                    context.browser_context_dir.clone(),
                    context.browser_profile_dir.clone(),
                    Some(BrowserOrigin {
                        target: options.target.clone(),
                        source_risk: options.source_risk,
                        source_label: options.source_label.clone(),
                    }),
                    options.allowlisted_domains.clone(),
                    Vec::new(),
                    None,
                ),
            )?;
        }

        return Ok(json!(ExtractCommandOutput {
            open: open_result,
            extract: extract_result,
            session_state: session.state,
        }));
    }

    if is_fixture_target(&options.target) {
        let catalog = load_fixture_catalog()?;
        let runtime = ReadOnlyRuntime::default();
        let vm = ReadOnlyActionVm::default();
        let mut session = runtime.start_session("scliextract001", DEFAULT_OPENED_AT);

        let open_result = vm.execute_fixture(
            &mut session,
            &catalog,
            ActionCommand {
                version: CONTRACT_VERSION.to_string(),
                action: ActionName::Open,
                target_ref: None,
                target_url: Some(options.target),
                risk_class: RiskClass::Low,
                reason: "Open a fixture-backed document before extraction.".to_string(),
                input: None,
            },
            DEFAULT_OPENED_AT,
        );
        let extract_result = if open_result.status == ActionStatus::Succeeded {
            let current_url = session.state.current_url.clone();
            let extract_result = vm.execute_fixture(
                &mut session,
                &catalog,
                ActionCommand {
                    version: CONTRACT_VERSION.to_string(),
                    action: ActionName::Extract,
                    target_ref: None,
                    target_url: current_url,
                    risk_class: RiskClass::Low,
                    reason: "Extract evidence for requested claims.".to_string(),
                    input: Some(json!({ "claims": claims })),
                },
                &slot_timestamp(1, 30),
            );
            verify_action_result_if_requested(
                extract_result,
                &mut session,
                &claims,
                options.verifier_command.as_deref(),
                &slot_timestamp(1, 30),
            )?
        } else {
            fail_action(
                ActionName::Extract,
                ActionFailureKind::InvalidInput,
                "Open step failed; extraction was not attempted.",
                open_result.policy.clone(),
            )
        };

        return Ok(json!(ExtractCommandOutput {
            open: open_result,
            extract: extract_result,
            session_state: session.state,
        }));
    }

    let runtime = ReadOnlyRuntime::default();
    let kernel = PolicyKernel;
    let mut acquisition = AcquisitionEngine::new(AcquisitionConfig::default())?;
    let mut session = runtime.start_session("scliextract001", DEFAULT_OPENED_AT);
    let source_risk = options.source_risk.unwrap_or(SourceRisk::Low);

    let snapshot = runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        source_risk,
        options.source_label,
        DEFAULT_OPENED_AT,
    )?;
    let open_policy =
        current_policy_with_allowlist(&session, &kernel, &options.allowlisted_domains);
    let open_result = succeed_action(
        ActionName::Open,
        "snapshot-document",
        json!(snapshot),
        "Opened live document.",
        open_policy.clone(),
    );

    let extract_timestamp = slot_timestamp(1, 30);
    let report = runtime.extract(&mut session, claims.clone(), &extract_timestamp)?;
    let extract_result = succeed_action(
        ActionName::Extract,
        "evidence-report",
        json!(report),
        "Extracted evidence report.",
        current_policy_with_allowlist(&session, &kernel, &options.allowlisted_domains),
    );
    let extract_result = verify_action_result_if_requested(
        extract_result,
        &mut session,
        &claims,
        options.verifier_command.as_deref(),
        &extract_timestamp,
    )?;

    Ok(json!(ExtractCommandOutput {
        open: open_result,
        extract: extract_result,
        session_state: session.state,
    }))
}

pub(crate) fn handle_policy(options: TargetOptions) -> Result<Value, CliError> {
    let kernel = PolicyKernel;

    if options.session_file.is_some() {
        return Err(CliError::Usage(
            "Use `session-policy --session-file <path>` for persisted browser sessions."
                .to_string(),
        ));
    }

    if options.browser {
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| browser_context_dir_for_session_file(path.as_path()))
            .map(|path| path.display().to_string());
        let context = open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir,
            None,
            "sclipolicy001",
            DEFAULT_OPENED_AT,
        )?;
        let report =
            current_policy_with_allowlist(&context.session, &kernel, &options.allowlisted_domains)
                .ok_or_else(|| {
                    CliError::Usage("Policy command requires an open snapshot.".to_string())
                })?;
        return Ok(json!(PolicyCommandOutput {
            policy: report,
            session_state: context.session.state,
        }));
    }

    if is_fixture_target(&options.target) {
        let catalog = load_fixture_catalog()?;
        let runtime = ReadOnlyRuntime::default();
        let mut session = runtime.start_session("sclipolicy001", DEFAULT_OPENED_AT);
        runtime.open(&mut session, &catalog, &options.target, DEFAULT_OPENED_AT)?;
        let report = current_policy_with_allowlist(&session, &kernel, &options.allowlisted_domains)
            .ok_or_else(|| {
                CliError::Usage("Policy command requires an open snapshot.".to_string())
            })?;
        return Ok(json!(PolicyCommandOutput {
            policy: report,
            session_state: session.state,
        }));
    }

    let runtime = ReadOnlyRuntime::default();
    let mut acquisition = AcquisitionEngine::new(AcquisitionConfig::default())?;
    let mut session = runtime.start_session("sclipolicy001", DEFAULT_OPENED_AT);
    runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.unwrap_or(SourceRisk::Low),
        options.source_label,
        DEFAULT_OPENED_AT,
    )?;
    let report = current_policy_with_allowlist(&session, &kernel, &options.allowlisted_domains)
        .ok_or_else(|| CliError::Usage("Policy command requires an open snapshot.".to_string()))?;

    Ok(json!(PolicyCommandOutput {
        policy: report,
        session_state: session.state,
    }))
}

pub(crate) fn handle_replay(scenario: &str) -> Result<Value, CliError> {
    let catalog = load_fixture_catalog()?;
    let runtime = ReadOnlyRuntime::default();
    let transcript_path = repo_root()
        .join("fixtures/scenarios")
        .join(scenario)
        .join("replay-transcript.json");
    let transcript: ReplayTranscript = serde_json::from_str(&fs::read_to_string(transcript_path)?)?;
    let session = runtime.replay(&catalog, &transcript, DEFAULT_OPENED_AT)?;

    Ok(json!(ReplayCommandOutput {
        session_state: session.state,
        replay_transcript: session.transcript,
        snapshot_count: session.snapshots.len(),
        evidence_report_count: session.evidence_reports.len(),
    }))
}

pub(crate) fn handle_memory_summary(steps: usize) -> Result<Value, CliError> {
    if steps == 0 || !steps.is_multiple_of(2) {
        return Err(CliError::Usage(
            "memory-summary requires an even `--steps` value greater than 0.".to_string(),
        ));
    }

    let runtime = ReadOnlyRuntime::default();
    let catalog = load_fixture_catalog()?;
    let mut session = runtime.start_session("sclimemory001", DEFAULT_OPENED_AT);
    let sequence = [
        (
            "fixture://research/static-docs/getting-started",
            "Touch Browser compiles web pages into semantic state for research agents.",
        ),
        (
            "fixture://research/citation-heavy/pricing",
            "The Starter plan costs $29 per month.",
        ),
        (
            "fixture://research/navigation/api-reference",
            "Snapshot responses include stable refs and evidence metadata.",
        ),
    ];

    let mut memory_refs = Vec::new();
    let mut memory_turns = Vec::new();

    for pair_index in 0..(steps / 2) {
        let step = sequence[pair_index % sequence.len()];
        let open_timestamp = slot_timestamp(pair_index * 2, 0);
        let extract_timestamp = slot_timestamp(pair_index * 2 + 1, 30);

        runtime
            .open(&mut session, &catalog, step.0, &open_timestamp)
            .map_err(CliError::Runtime)?;
        let snapshot_record = session
            .current_snapshot_record()
            .expect("open should create a current snapshot");
        let open_turn = plan_memory_turn(
            memory_turns.len() + 1,
            &snapshot_record.snapshot_id,
            &snapshot_record.snapshot,
            None,
            &memory_refs,
            6,
        );
        memory_refs = open_turn.kept_refs.clone();
        memory_turns.push(open_turn);

        runtime
            .extract(
                &mut session,
                vec![ClaimInput {
                    id: format!("c{}", pair_index + 1),
                    statement: step.1.to_string(),
                }],
                &extract_timestamp,
            )
            .map_err(CliError::Runtime)?;
        let snapshot_record = session
            .current_snapshot_record()
            .expect("extract should retain the current snapshot");
        let report = session
            .evidence_reports
            .last()
            .expect("extract should create an evidence report");
        let extract_turn = plan_memory_turn(
            memory_turns.len() + 1,
            &snapshot_record.snapshot_id,
            &snapshot_record.snapshot,
            Some(&report.report),
            &memory_refs,
            6,
        );
        memory_refs = extract_turn.kept_refs.clone();
        memory_turns.push(extract_turn);
    }

    Ok(json!(MemorySummaryOutput {
        requested_actions: steps,
        action_count: steps,
        session_state: session.state,
        memory_summary: summarize_turns(&memory_turns, 12),
    }))
}
