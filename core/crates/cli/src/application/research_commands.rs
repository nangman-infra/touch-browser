use std::{fs, path::Path};

use super::{
    context::CliAppContext,
    deps::{
        browser_capture_diagnostics, browser_fallback_reason, current_policy_with_allowlist,
        current_timestamp, fail_action, http_capture_diagnostics, is_fixture_target,
        plan_memory_turn, repo_root, slot_timestamp, succeed_action, summarize_turns,
        verify_action_result_if_requested, ActionCommand, ActionFailureKind, ActionName,
        ActionResult, ActionStatus, BrowserCliSession, BrowserOrigin, BrowserSessionContext,
        CaptureSurface, ClaimInput, CliError, CompactSnapshotOutput, ExtractCommandOutput,
        ExtractOptions, MemorySummaryOutput, PolicyCommandOutput, ReadViewOutput,
        ReplayCommandOutput, ReplayTranscript, RiskClass, SearchActionActor, SearchActionHint,
        SearchCommandOutput, SearchEngine, SearchNextCommands, SearchOpenResultCommandOutput,
        SearchOpenResultOptions, SearchOpenTopCommandOutput, SearchOpenTopItem,
        SearchOpenTopOptions, SearchOptions, SearchRecovery, SearchRecoveryAttempt, SearchReport,
        SearchReportStatus, SearchResultItem, SourceRisk, TargetOptions, CONTRACT_VERSION,
        DEFAULT_OPENED_AT,
    },
    search_support::{
        build_search_report, build_search_url, derived_search_result_session_file,
        load_preferred_search_engine, record_search_profile_result, resolve_search_profile_dir,
        resolve_search_session_file, save_preferred_search_engine, search_engine_source_label,
    },
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ExtractActionInput<'a> {
    claims: &'a [ClaimInput],
}

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
            "extract requires at least one non-empty `--claim` statement.".to_string(),
        ));
    }
    Ok(claims)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn search_retry_command(
    query: &str,
    engine: SearchEngine,
    headed: bool,
    session_file: &str,
) -> String {
    let engine_value = match engine {
        SearchEngine::Google => "google",
        SearchEngine::Brave => "brave",
    };
    let headed_flag = if headed { " --headed" } else { "" };
    let session_file_flag = if should_include_search_session_file(engine, session_file) {
        format!(" --session-file {}", shell_quote(session_file))
    } else {
        String::new()
    };
    format!(
        "touch-browser search {} --engine {}{}{}",
        shell_quote(query),
        engine_value,
        session_file_flag,
        headed_flag
    )
}

fn should_include_search_session_file(engine: SearchEngine, session_file: &str) -> bool {
    let default_session_file = resolve_search_session_file(None, engine);
    Path::new(session_file) != default_session_file.as_path()
}

fn merged_allowlisted_domains(existing: &[String], added: &[String]) -> Vec<String> {
    let mut domains = existing
        .iter()
        .chain(added.iter())
        .filter(|domain| !domain.trim().is_empty())
        .cloned()
        .collect::<Vec<_>>();
    domains.sort();
    domains.dedup();
    domains
}

#[allow(clippy::too_many_arguments)]
fn persist_browser_context(
    ctx: &CliAppContext<'_>,
    session_file: &Path,
    context: &BrowserSessionContext,
    target: &str,
    headed: bool,
    allowlisted_domains: &[String],
    latest_search: Option<SearchReport>,
) -> Result<BrowserCliSession, CliError> {
    let ports = ctx.ports;
    if session_file.exists() {
        let mut persisted = ports.session_store.load_session(session_file)?;
        let timestamp = ports.browser.next_session_timestamp(&persisted.session);
        context.runtime.open_snapshot(
            &mut persisted.session,
            target,
            context.snapshot.clone(),
            context.source_risk.clone(),
            context.source_label.clone(),
            &timestamp,
        )?;
        persisted.version = CONTRACT_VERSION.to_string();
        persisted.headless = !headed;
        persisted.requested_budget = context.snapshot.budget.requested_tokens;
        persisted.browser_state = Some(context.browser_state.clone());
        persisted.browser_context_dir = context.browser_context_dir.clone();
        persisted.browser_profile_dir = context.browser_profile_dir.clone();
        persisted.browser_origin = Some(BrowserOrigin {
            target: target.to_string(),
            source_risk: Some(context.source_risk.clone()),
            source_label: context.source_label.clone(),
        });
        persisted.allowlisted_domains =
            merged_allowlisted_domains(&persisted.allowlisted_domains, allowlisted_domains);
        if let Some(latest_search) = latest_search {
            persisted.latest_search = Some(latest_search);
        }
        ports.session_store.save_session(session_file, &persisted)?;
        return Ok(persisted);
    }

    let persisted = ports.browser.build_browser_cli_session(
        &context.session,
        context.snapshot.budget.requested_tokens,
        !headed,
        Some(context.browser_state.clone()),
        context.browser_context_dir.clone(),
        context.browser_profile_dir.clone(),
        Some(BrowserOrigin {
            target: target.to_string(),
            source_risk: Some(context.source_risk.clone()),
            source_label: context.source_label.clone(),
        }),
        allowlisted_domains.to_vec(),
        Vec::new(),
        latest_search,
    );
    ports.session_store.save_session(session_file, &persisted)?;
    Ok(persisted)
}

fn target_requires_browser_session(options: &TargetOptions) -> bool {
    options.browser || options.session_file.is_some()
}

fn extract_requires_browser_session(options: &ExtractOptions) -> bool {
    options.browser || options.session_file.is_some()
}

fn browser_fallback_target_options(options: &TargetOptions) -> TargetOptions {
    let mut fallback = options.clone();
    fallback.browser = true;
    fallback
}

fn browser_fallback_extract_options(options: &ExtractOptions) -> ExtractOptions {
    let mut fallback = options.clone();
    fallback.browser = true;
    fallback
}

pub(crate) fn selected_search_result(
    latest_search: &SearchReport,
    requested_rank: usize,
    prefer_official: bool,
) -> Result<(SearchResultItem, &'static str), CliError> {
    if prefer_official {
        let official_results = latest_search
            .results
            .iter()
            .filter(|result| result.official_likely)
            .cloned()
            .collect::<Vec<_>>();
        if let Some(selected) = official_results.get(requested_rank.saturating_sub(1)) {
            return Ok((selected.clone(), "prefer-official"));
        }
        if !official_results.is_empty() {
            return Err(CliError::Usage(format!(
                "Official-like search results do not contain rank {}.",
                requested_rank
            )));
        }
    }

    latest_search
        .results
        .iter()
        .find(|result| result.rank == requested_rank)
        .cloned()
        .map(|selected| (selected, "rank"))
        .ok_or_else(|| {
            CliError::Usage(format!(
                "Saved search results do not contain rank {}.",
                requested_rank
            ))
        })
}

fn should_auto_recover_search(options: &SearchOptions) -> bool {
    !options.engine_explicit && options.session_file.is_none()
}

fn alternate_search_engine(engine: SearchEngine) -> SearchEngine {
    match engine {
        SearchEngine::Google => SearchEngine::Brave,
        SearchEngine::Brave => SearchEngine::Google,
    }
}

fn resolved_search_engine(options: &SearchOptions) -> Result<SearchEngine, CliError> {
    if should_auto_recover_search(options) {
        return Ok(load_preferred_search_engine()?.unwrap_or(options.engine));
    }

    Ok(options.engine)
}

fn remember_successful_search_engine(
    engine: SearchEngine,
    status: SearchReportStatus,
) -> Result<(), CliError> {
    if matches!(
        status,
        SearchReportStatus::Ready | SearchReportStatus::NoResults
    ) {
        save_preferred_search_engine(engine)?;
    }
    Ok(())
}

fn recovery_attempt_from_report(report: &SearchReport) -> SearchRecoveryAttempt {
    SearchRecoveryAttempt {
        engine: report.engine,
        status: report.status,
        status_detail: report.status_detail.clone(),
    }
}

fn google_challenge_hint(query: &str, google_session_file: &str) -> Vec<SearchActionHint> {
    vec![
        SearchActionHint {
            action: "retry-google-headed".to_string(),
            detail: "If you specifically want Google, run the same Google search in headed mode once, clear the challenge manually, then keep reusing that saved Google profile.".to_string(),
            actor: SearchActionActor::Human,
            engine: Some(SearchEngine::Google),
            command: Some(search_retry_command(
                query,
                SearchEngine::Google,
                true,
                google_session_file,
            )),
            can_auto_run: false,
            headed_required: true,
            result_ranks: Vec::new(),
        },
        SearchActionHint {
            action: "resume-google-search".to_string(),
            detail: "After the manual Google challenge is cleared, rerun the same Google search without headed mode to keep using the warmed Google profile automatically.".to_string(),
            actor: SearchActionActor::Ai,
            engine: Some(SearchEngine::Google),
            command: Some(search_retry_command(
                query,
                SearchEngine::Google,
                false,
                google_session_file,
            )),
            can_auto_run: true,
            headed_required: false,
            result_ranks: Vec::new(),
        },
    ]
}

fn challenge_hints_for_engine(
    query: &str,
    engine: SearchEngine,
    session_file: &str,
) -> Vec<SearchActionHint> {
    vec![
        SearchActionHint {
            action: "complete-challenge".to_string(),
            detail: "The current search provider returned a challenge page. Re-run the same search in headed mode, clear the challenge manually once on the same saved profile, then rerun it without headed mode.".to_string(),
            actor: SearchActionActor::Human,
            engine: Some(engine),
            command: Some(search_retry_command(query, engine, true, session_file)),
            can_auto_run: false,
            headed_required: true,
            result_ranks: Vec::new(),
        },
        SearchActionHint {
            action: "resume-search".to_string(),
            detail: "After the manual challenge is cleared, rerun the same search against the same saved profile without headed mode.".to_string(),
            actor: SearchActionActor::Ai,
            engine: Some(engine),
            command: Some(search_retry_command(query, engine, false, session_file)),
            can_auto_run: true,
            headed_required: false,
            result_ranks: Vec::new(),
        },
    ]
}

fn enrich_search_output_with_recovery(
    mut output: SearchCommandOutput,
    attempts: &[SearchRecoveryAttempt],
) -> SearchCommandOutput {
    let google_session_file = resolve_search_session_file(None, SearchEngine::Google)
        .display()
        .to_string();
    let final_engine = output.result.engine;
    let recovered = attempts.len() > 1
        && attempts
            .iter()
            .any(|attempt| attempt.status == SearchReportStatus::Challenge)
        && matches!(
            output.result.status,
            SearchReportStatus::Ready | SearchReportStatus::NoResults
        );
    let human_intervention_required_now = output.result.status == SearchReportStatus::Challenge;
    let recovery = (!attempts.is_empty()).then(|| SearchRecovery {
        recovered,
        human_intervention_required_now,
        final_engine,
        attempts: attempts.to_vec(),
    });

    output.result.recovery = recovery.clone();
    output.search.recovery = recovery;

    let final_session_file = output.session_file.clone();
    if output.result.status == SearchReportStatus::Challenge {
        output
            .result
            .next_action_hints
            .retain(|hint| hint.action != "complete-challenge");
        output
            .result
            .next_action_hints
            .extend(challenge_hints_for_engine(
                &output.query,
                final_engine,
                &final_session_file,
            ));
    }

    if attempts.iter().any(|attempt| {
        attempt.engine == SearchEngine::Google && attempt.status == SearchReportStatus::Challenge
    }) && final_engine != SearchEngine::Google
    {
        output
            .result
            .next_action_hints
            .extend(google_challenge_hint(&output.query, &google_session_file));
    }

    output.search.next_action_hints = output.result.next_action_hints.clone();
    output
}

fn execute_search(
    ctx: &CliAppContext<'_>,
    options: &SearchOptions,
    engine: SearchEngine,
    session_file: std::path::PathBuf,
) -> Result<SearchCommandOutput, CliError> {
    let ports = ctx.ports;
    let opened_at = current_timestamp();
    let search_url = build_search_url(engine, &options.query)?;
    let resolved_profile_dir = resolve_search_profile_dir(options.profile_dir.as_ref(), engine);
    let browser_profile_dir = Some(resolved_profile_dir.display().to_string());
    let browser_context_dir = None;
    let context = ports.browser.open_browser_session(
        &search_url,
        options.budget,
        Some(SourceRisk::Low),
        Some(search_engine_source_label(engine).to_string()),
        options.headed,
        browser_context_dir.clone(),
        browser_profile_dir.clone(),
        "sclisearch001",
        &opened_at,
    )?;
    let report = build_search_report(
        engine,
        &options.query,
        &search_url,
        &context.snapshot,
        &context.browser_state.current_html,
        &context.browser_state.current_url,
        &opened_at,
    );
    record_search_profile_result(
        engine,
        &resolved_profile_dir,
        report.status,
        options.headed,
        &opened_at,
    )?;

    ports.session_store.save_session(
        &session_file,
        &ports.browser.build_browser_cli_session(
            &context.session,
            context.snapshot.budget.requested_tokens,
            !options.headed,
            Some(context.browser_state.clone()),
            context.browser_context_dir.clone(),
            context.browser_profile_dir.clone(),
            Some(BrowserOrigin {
                target: search_url.clone(),
                source_risk: Some(SourceRisk::Low),
                source_label: Some(search_engine_source_label(engine).to_string()),
            }),
            Vec::new(),
            Vec::new(),
            Some(report.clone()),
        ),
    )?;

    Ok(SearchCommandOutput {
        query: options.query.clone(),
        engine,
        search_url,
        result_count: report.result_count,
        search: report.clone(),
        result: report,
        browser_context_dir,
        browser_profile_dir,
        session_state: context.session.state,
        session_file: session_file.display().to_string(),
    })
}

pub(crate) fn handle_search(
    ctx: &CliAppContext<'_>,
    options: SearchOptions,
) -> Result<SearchCommandOutput, CliError> {
    let auto_recover = should_auto_recover_search(&options);
    let primary_engine = resolved_search_engine(&options)?;
    let primary_session_file =
        resolve_search_session_file(options.session_file.as_ref(), primary_engine);
    let primary = execute_search(ctx, &options, primary_engine, primary_session_file)?;
    let primary_attempt = recovery_attempt_from_report(&primary.result);
    if primary.result.status != SearchReportStatus::Challenge || !auto_recover {
        remember_successful_search_engine(primary.engine, primary.result.status)?;
        return Ok(enrich_search_output_with_recovery(
            primary,
            &[primary_attempt],
        ));
    }

    let fallback_engine = alternate_search_engine(primary.engine);
    let fallback = execute_search(
        ctx,
        &options,
        fallback_engine,
        resolve_search_session_file(None, fallback_engine),
    )?;
    let fallback_attempt = recovery_attempt_from_report(&fallback.result);
    remember_successful_search_engine(fallback.engine, fallback.result.status)?;
    Ok(enrich_search_output_with_recovery(
        fallback,
        &[primary_attempt, fallback_attempt],
    ))
}

pub(crate) fn handle_search_open_result(
    ctx: &CliAppContext<'_>,
    options: SearchOpenResultOptions,
) -> Result<SearchOpenResultCommandOutput, CliError> {
    let ports = ctx.ports;
    let session_file = resolve_search_session_file(options.session_file.as_ref(), options.engine);
    let persisted = ports.session_store.load_session(&session_file)?;
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
    let (selected, selection_strategy) =
        selected_search_result(&latest_search, options.rank, options.prefer_official)?;
    let opened_at = current_timestamp();

    let context = ports.browser.open_browser_session(
        &selected.url,
        persisted.requested_budget,
        Some(SourceRisk::Low),
        None,
        options.headed,
        persisted.browser_context_dir.clone(),
        persisted.browser_profile_dir.clone(),
        "scliopen001",
        &opened_at,
    )?;
    let refreshed = persist_browser_context(
        ctx,
        &session_file,
        &context,
        &selected.url,
        options.headed,
        &persisted.allowlisted_domains,
        Some(latest_search.clone()),
    )?;
    let diagnostics = browser_capture_diagnostics(
        &context.snapshot,
        persisted.requested_budget,
        false,
        None,
        &context.load_diagnostics,
        CaptureSurface::Open,
    );
    let mut opened = succeed_action(
        ActionName::Open,
        "snapshot-document",
        context.snapshot,
        "Opened browser-backed document.",
        current_policy_with_allowlist(
            &refreshed.session,
            ctx.policy_kernel,
            &refreshed.allowlisted_domains,
        ),
    )?;
    opened.diagnostics = Some(diagnostics.clone());

    let session_extract_hint =
        if session_file == resolve_search_session_file(None, latest_search.engine) {
            format!(
                "touch-browser session-extract --engine {} --claim \"<statement>\"",
                match latest_search.engine {
                    SearchEngine::Google => "google",
                    SearchEngine::Brave => "brave",
                }
            )
        } else {
            format!(
                "touch-browser session-extract --session-file {} --claim \"<statement>\"",
                shell_quote(&session_file.display().to_string())
            )
        };

    Ok(SearchOpenResultCommandOutput {
        selected_result: selected,
        requested_rank: options.rank,
        selection_strategy: selection_strategy.to_string(),
        result: opened,
        diagnostics: Some(diagnostics),
        session_file: session_file.display().to_string(),
        next_commands: SearchNextCommands {
            session_extract: session_extract_hint,
            session_read: format!(
                "touch-browser session-read --session-file {} --main-only",
                shell_quote(&session_file.display().to_string())
            ),
        },
    })
}

pub(crate) fn handle_search_open_top(
    ctx: &CliAppContext<'_>,
    options: SearchOpenTopOptions,
) -> Result<SearchOpenTopCommandOutput, CliError> {
    let ports = ctx.ports;
    let search_session_file =
        resolve_search_session_file(options.session_file.as_ref(), options.engine);
    let persisted = ports.session_store.load_session(&search_session_file)?;
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
            let opened_at = current_timestamp();
            let context = ports.browser.open_browser_session(
                &selected.url,
                persisted.requested_budget,
                Some(SourceRisk::Low),
                None,
                options.headed,
                persisted.browser_context_dir.clone(),
                persisted.browser_profile_dir.clone(),
                "scliopen001",
                &opened_at,
            )?;
            let refreshed = persist_browser_context(
                ctx,
                &result_session_file,
                &context,
                &selected.url,
                options.headed,
                &persisted.allowlisted_domains,
                None,
            )?;
            let diagnostics = browser_capture_diagnostics(
                &context.snapshot,
                persisted.requested_budget,
                false,
                None,
                &context.load_diagnostics,
                CaptureSurface::Open,
            );
            let mut opened = succeed_action(
                ActionName::Open,
                "snapshot-document",
                context.snapshot,
                "Opened browser-backed document.",
                current_policy_with_allowlist(
                    &refreshed.session,
                    ctx.policy_kernel,
                    &refreshed.allowlisted_domains,
                ),
            )?;
            opened.diagnostics = Some(diagnostics.clone());

            Ok::<SearchOpenTopItem, CliError>(SearchOpenTopItem {
                rank: selected.rank,
                selected_result: selected,
                session_file: result_session_file.display().to_string(),
                result: opened,
                diagnostics: Some(diagnostics),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(SearchOpenTopCommandOutput {
        search_session_file: search_session_file.display().to_string(),
        opened_count: opened.len(),
        opened,
    })
}

pub(crate) fn handle_open(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<ActionResult, CliError> {
    let ports = ctx.ports;
    let opened_at = current_timestamp();
    if target_requires_browser_session(&options) {
        return handle_browser_open(ctx, options);
    }

    if is_fixture_target(&options.target) {
        let catalog = ports.fixtures.load_catalog()?;
        let mut session = ctx.runtime.start_session("scliopen001", DEFAULT_OPENED_AT);
        let result = ctx.action_vm.execute_fixture(
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
        return Ok(result);
    }

    let mut acquisition = ports.acquisition.create_engine()?;
    let mut session = ctx.runtime.start_session("scliopen001", &opened_at);
    let source_risk = options.source_risk.clone().unwrap_or(SourceRisk::Low);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        source_risk.clone(),
        options.source_label.clone(),
        &opened_at,
    )?;
    if let Some(reason) = browser_fallback_reason(&snapshot) {
        return handle_browser_open_with_fallback(
            ctx,
            browser_fallback_target_options(&options),
            Some(reason),
        );
    }
    let policy =
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains);
    let diagnostics = http_capture_diagnostics(&snapshot, options.budget, CaptureSurface::Open);
    let mut result = succeed_action(
        ActionName::Open,
        "snapshot-document",
        snapshot,
        "Opened live document.",
        policy,
    )?;
    result.diagnostics = Some(diagnostics);
    Ok(result)
}

pub(crate) fn handle_browser_open(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<ActionResult, CliError> {
    handle_browser_open_with_fallback(ctx, options, None)
}

fn handle_browser_open_with_fallback(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
    fallback_reason: Option<&str>,
) -> Result<ActionResult, CliError> {
    let ports = ctx.ports;
    let opened_at = current_timestamp();
    let browser_context_dir = options
        .session_file
        .as_ref()
        .map(|path| {
            ports
                .session_store
                .browser_context_dir_for_session(path.as_path())
        })
        .map(|path: std::path::PathBuf| path.display().to_string());
    let context = ports.browser.open_browser_session(
        &options.target,
        options.budget,
        options.source_risk.clone(),
        options.source_label.clone(),
        options.headed,
        browser_context_dir.clone(),
        None,
        "scliopen001",
        &opened_at,
    )?;
    let persisted = options
        .session_file
        .as_ref()
        .map(|session_file| {
            persist_browser_context(
                ctx,
                session_file,
                &context,
                &options.target,
                options.headed,
                &options.allowlisted_domains,
                None,
            )
        })
        .transpose()?;
    let diagnostics = browser_capture_diagnostics(
        &context.snapshot,
        options.budget,
        fallback_reason.is_some(),
        fallback_reason,
        &context.load_diagnostics,
        CaptureSurface::Open,
    );
    let mut result = succeed_action(
        ActionName::Open,
        "snapshot-document",
        context.snapshot,
        "Opened browser-backed document.",
        match persisted.as_ref() {
            Some(persisted) => current_policy_with_allowlist(
                &persisted.session,
                ctx.policy_kernel,
                &persisted.allowlisted_domains,
            ),
            None => current_policy_with_allowlist(
                &context.session,
                ctx.policy_kernel,
                &options.allowlisted_domains,
            ),
        },
    )?;
    result.diagnostics = Some(diagnostics);
    Ok(result)
}

pub(crate) fn handle_compact_view(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<CompactSnapshotOutput, CliError> {
    let ports = ctx.ports;
    if target_requires_browser_session(&options) {
        let opened_at = current_timestamp();
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| {
                ports
                    .session_store
                    .browser_context_dir_for_session(path.as_path())
            })
            .map(|path: std::path::PathBuf| path.display().to_string());
        let context = ports.browser.open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir.clone(),
            None,
            "sclicompact001",
            &opened_at,
        )?;

        let persisted = options
            .session_file
            .as_ref()
            .map(|session_file| {
                persist_browser_context(
                    ctx,
                    session_file,
                    &context,
                    &options.target,
                    options.headed,
                    &options.allowlisted_domains,
                    None,
                )
            })
            .transpose()?;

        return Ok(CompactSnapshotOutput::new(
            &context.snapshot,
            Some(
                persisted
                    .as_ref()
                    .map(|session| session.session.state.clone())
                    .unwrap_or(context.session.state),
            ),
            options.session_file.map(|path| path.display().to_string()),
        ));
    }

    if is_fixture_target(&options.target) {
        let catalog = ports.fixtures.load_catalog()?;
        let mut session = ctx
            .runtime
            .start_session("sclicompact001", DEFAULT_OPENED_AT);
        let snapshot =
            ctx.runtime
                .open(&mut session, &catalog, &options.target, DEFAULT_OPENED_AT)?;
        return Ok(CompactSnapshotOutput::new(
            &snapshot,
            Some(session.state),
            None,
        ));
    }

    let mut acquisition = ports.acquisition.create_engine()?;
    let opened_at = current_timestamp();
    let mut session = ctx.runtime.start_session("sclicompact001", &opened_at);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.clone().unwrap_or(SourceRisk::Low),
        options.source_label.clone(),
        &opened_at,
    )?;
    if browser_fallback_reason(&snapshot).is_some() {
        return handle_compact_view(ctx, browser_fallback_target_options(&options));
    }

    Ok(CompactSnapshotOutput::new(
        &snapshot,
        Some(session.state),
        None,
    ))
}

pub(crate) fn handle_read_view(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<ReadViewOutput, CliError> {
    let ports = ctx.ports;
    if target_requires_browser_session(&options) {
        let opened_at = current_timestamp();
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| {
                ports
                    .session_store
                    .browser_context_dir_for_session(path.as_path())
            })
            .map(|path: std::path::PathBuf| path.display().to_string());
        let context = ports.browser.open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir.clone(),
            None,
            "scliread001",
            &opened_at,
        )?;

        let persisted = options
            .session_file
            .as_ref()
            .map(|session_file| {
                persist_browser_context(
                    ctx,
                    session_file,
                    &context,
                    &options.target,
                    options.headed,
                    &options.allowlisted_domains,
                    None,
                )
            })
            .transpose()?;

        let diagnostics = browser_capture_diagnostics(
            &context.snapshot,
            options.budget,
            false,
            None,
            &context.load_diagnostics,
            CaptureSurface::ReadView,
        );
        return Ok(ReadViewOutput::new(
            &context.snapshot,
            Some(
                persisted
                    .as_ref()
                    .map(|session| session.session.state.clone())
                    .unwrap_or(context.session.state),
            ),
            options.session_file.map(|path| path.display().to_string()),
            options.main_only,
        )
        .with_diagnostics(diagnostics));
    }

    if is_fixture_target(&options.target) {
        let catalog = ports.fixtures.load_catalog()?;
        let mut session = ctx.runtime.start_session("scliread001", DEFAULT_OPENED_AT);
        let snapshot =
            ctx.runtime
                .open(&mut session, &catalog, &options.target, DEFAULT_OPENED_AT)?;
        return Ok(ReadViewOutput::new(
            &snapshot,
            Some(session.state),
            None,
            options.main_only,
        ));
    }

    let mut acquisition = ports.acquisition.create_engine()?;
    let opened_at = current_timestamp();
    let mut session = ctx.runtime.start_session("scliread001", &opened_at);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.clone().unwrap_or(SourceRisk::Low),
        options.source_label.clone(),
        &opened_at,
    )?;
    if let Some(reason) = browser_fallback_reason(&snapshot) {
        return handle_browser_read_view_with_fallback(
            ctx,
            browser_fallback_target_options(&options),
            Some(reason),
        );
    }
    let diagnostics = http_capture_diagnostics(&snapshot, options.budget, CaptureSurface::ReadView);
    Ok(
        ReadViewOutput::new(&snapshot, Some(session.state), None, options.main_only)
            .with_diagnostics(diagnostics),
    )
}

fn handle_browser_read_view_with_fallback(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
    fallback_reason: Option<&str>,
) -> Result<ReadViewOutput, CliError> {
    let ports = ctx.ports;
    let opened_at = current_timestamp();
    let browser_context_dir = options
        .session_file
        .as_ref()
        .map(|path| {
            ports
                .session_store
                .browser_context_dir_for_session(path.as_path())
        })
        .map(|path: std::path::PathBuf| path.display().to_string());
    let context = ports.browser.open_browser_session(
        &options.target,
        options.budget,
        options.source_risk.clone(),
        options.source_label.clone(),
        options.headed,
        browser_context_dir.clone(),
        None,
        "scliread001",
        &opened_at,
    )?;

    let persisted = options
        .session_file
        .as_ref()
        .map(|session_file| {
            persist_browser_context(
                ctx,
                session_file,
                &context,
                &options.target,
                options.headed,
                &options.allowlisted_domains,
                None,
            )
        })
        .transpose()?;
    let diagnostics = browser_capture_diagnostics(
        &context.snapshot,
        options.budget,
        fallback_reason.is_some(),
        fallback_reason,
        &context.load_diagnostics,
        CaptureSurface::ReadView,
    );

    Ok(ReadViewOutput::new(
        &context.snapshot,
        Some(
            persisted
                .as_ref()
                .map(|session| session.session.state.clone())
                .unwrap_or(context.session.state),
        ),
        options.session_file.map(|path| path.display().to_string()),
        options.main_only,
    )
    .with_diagnostics(diagnostics))
}

pub(crate) fn handle_extract(
    ctx: &CliAppContext<'_>,
    options: ExtractOptions,
) -> Result<ExtractCommandOutput, CliError> {
    let ports = ctx.ports;
    let claims = claim_inputs_from_statements(&options.claims)?;

    if extract_requires_browser_session(&options) {
        let opened_at = current_timestamp();
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| {
                ports
                    .session_store
                    .browser_context_dir_for_session(path.as_path())
            })
            .map(|path: std::path::PathBuf| path.display().to_string());
        let context = ports.browser.open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir.clone(),
            None,
            "scliextract001",
            &opened_at,
        )?;
        let persisted_session = options
            .session_file
            .as_ref()
            .map(|session_file| {
                persist_browser_context(
                    ctx,
                    session_file,
                    &context,
                    &options.target,
                    options.headed,
                    &options.allowlisted_domains,
                    None,
                )
            })
            .transpose()?;
        let diagnostics = browser_capture_diagnostics(
            &context.snapshot,
            options.budget,
            false,
            None,
            &context.load_diagnostics,
            CaptureSurface::Extract,
        );
        let mut open_result = succeed_action(
            ActionName::Open,
            "snapshot-document",
            context.snapshot.clone(),
            "Opened browser-backed document.",
            match persisted_session.as_ref() {
                Some(persisted) => current_policy_with_allowlist(
                    &persisted.session,
                    ctx.policy_kernel,
                    &persisted.allowlisted_domains,
                ),
                None => current_policy_with_allowlist(
                    &context.session,
                    ctx.policy_kernel,
                    &options.allowlisted_domains,
                ),
            },
        )?;
        open_result.diagnostics = Some(diagnostics.clone());
        let mut session = persisted_session
            .as_ref()
            .map(|persisted| persisted.session.clone())
            .unwrap_or_else(|| context.session.clone());
        let extract_timestamp = current_timestamp();
        let report = context
            .runtime
            .extract(&mut session, claims.clone(), &extract_timestamp)?;
        let extract_result = succeed_action(
            ActionName::Extract,
            "evidence-report",
            report,
            "Extracted evidence report from browser-backed snapshot.",
            current_policy_with_allowlist(
                &session,
                ctx.policy_kernel,
                &options.allowlisted_domains,
            ),
        );
        let extract_result = verify_action_result_if_requested(
            ports.verifier,
            extract_result?,
            &mut session,
            &claims,
            options.verifier_command.as_deref(),
            &extract_timestamp,
        )?;
        let persisted = if let Some(session_file) = options.session_file.as_ref() {
            let mut persisted = persisted_session
                .expect("persisted browser session should exist when session file is provided");
            persisted.session = session.clone();
            ports.session_store.save_session(session_file, &persisted)?;
            Some(persisted)
        } else {
            None
        };

        return Ok(ExtractCommandOutput {
            open: open_result,
            extract: extract_result,
            diagnostics: Some(diagnostics),
            session_state: persisted
                .as_ref()
                .map(|persisted| persisted.session.state.clone())
                .unwrap_or(session.state),
        });
    }

    if is_fixture_target(&options.target) {
        let catalog = ports.fixtures.load_catalog()?;
        let mut session = ctx
            .runtime
            .start_session("scliextract001", DEFAULT_OPENED_AT);

        let open_result = ctx.action_vm.execute_fixture(
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
            let extract_result = ctx.action_vm.execute_fixture(
                &mut session,
                &catalog,
                ActionCommand {
                    version: CONTRACT_VERSION.to_string(),
                    action: ActionName::Extract,
                    target_ref: None,
                    target_url: current_url,
                    risk_class: RiskClass::Low,
                    reason: "Extract evidence for requested claims.".to_string(),
                    input: Some(serde_json::to_value(ExtractActionInput {
                        claims: &claims,
                    })?),
                },
                &slot_timestamp(1, 30),
            );
            verify_action_result_if_requested(
                ports.verifier,
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

        return Ok(ExtractCommandOutput {
            open: open_result,
            extract: extract_result,
            diagnostics: None,
            session_state: session.state,
        });
    }

    let mut acquisition = ports.acquisition.create_engine()?;
    let opened_at = current_timestamp();
    let mut session = ctx.runtime.start_session("scliextract001", &opened_at);
    let source_risk = options.source_risk.clone().unwrap_or(SourceRisk::Low);

    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        source_risk,
        options.source_label.clone(),
        &opened_at,
    )?;
    if let Some(reason) = browser_fallback_reason(&snapshot) {
        return handle_browser_extract_with_fallback(
            ctx,
            browser_fallback_extract_options(&options),
            Some(reason),
        );
    }
    let open_policy =
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains);
    let diagnostics = http_capture_diagnostics(&snapshot, options.budget, CaptureSurface::Extract);
    let mut open_result = succeed_action(
        ActionName::Open,
        "snapshot-document",
        snapshot,
        "Opened live document.",
        open_policy.clone(),
    )?;
    open_result.diagnostics = Some(diagnostics.clone());

    let extract_timestamp = current_timestamp();
    let report = ctx
        .runtime
        .extract(&mut session, claims.clone(), &extract_timestamp)?;
    let extract_result = succeed_action(
        ActionName::Extract,
        "evidence-report",
        report,
        "Extracted evidence report.",
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains),
    );
    let extract_result = verify_action_result_if_requested(
        ports.verifier,
        extract_result?,
        &mut session,
        &claims,
        options.verifier_command.as_deref(),
        &extract_timestamp,
    )?;

    Ok(ExtractCommandOutput {
        open: open_result,
        extract: extract_result,
        diagnostics: Some(diagnostics),
        session_state: session.state,
    })
}

fn handle_browser_extract_with_fallback(
    ctx: &CliAppContext<'_>,
    options: ExtractOptions,
    fallback_reason: Option<&str>,
) -> Result<ExtractCommandOutput, CliError> {
    let ports = ctx.ports;
    let claims = claim_inputs_from_statements(&options.claims)?;
    let opened_at = current_timestamp();
    let browser_context_dir = options
        .session_file
        .as_ref()
        .map(|path| {
            ports
                .session_store
                .browser_context_dir_for_session(path.as_path())
        })
        .map(|path: std::path::PathBuf| path.display().to_string());
    let context = ports.browser.open_browser_session(
        &options.target,
        options.budget,
        options.source_risk.clone(),
        options.source_label.clone(),
        options.headed,
        browser_context_dir.clone(),
        None,
        "scliextract001",
        &opened_at,
    )?;
    let persisted_session = options
        .session_file
        .as_ref()
        .map(|session_file| {
            persist_browser_context(
                ctx,
                session_file,
                &context,
                &options.target,
                options.headed,
                &options.allowlisted_domains,
                None,
            )
        })
        .transpose()?;
    let diagnostics = browser_capture_diagnostics(
        &context.snapshot,
        options.budget,
        fallback_reason.is_some(),
        fallback_reason,
        &context.load_diagnostics,
        CaptureSurface::Extract,
    );
    let mut open_result = succeed_action(
        ActionName::Open,
        "snapshot-document",
        context.snapshot.clone(),
        "Opened browser-backed document.",
        match persisted_session.as_ref() {
            Some(persisted) => current_policy_with_allowlist(
                &persisted.session,
                ctx.policy_kernel,
                &persisted.allowlisted_domains,
            ),
            None => current_policy_with_allowlist(
                &context.session,
                ctx.policy_kernel,
                &options.allowlisted_domains,
            ),
        },
    )?;
    open_result.diagnostics = Some(diagnostics.clone());
    let mut session = persisted_session
        .as_ref()
        .map(|persisted| persisted.session.clone())
        .unwrap_or_else(|| context.session.clone());
    let extract_timestamp = current_timestamp();
    let report = context
        .runtime
        .extract(&mut session, claims.clone(), &extract_timestamp)?;
    let extract_result = succeed_action(
        ActionName::Extract,
        "evidence-report",
        report,
        "Extracted evidence report from browser-backed snapshot.",
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains),
    );
    let extract_result = verify_action_result_if_requested(
        ports.verifier,
        extract_result?,
        &mut session,
        &claims,
        options.verifier_command.as_deref(),
        &extract_timestamp,
    )?;
    let persisted = if let Some(session_file) = options.session_file.as_ref() {
        let mut persisted = persisted_session
            .expect("persisted browser session should exist when session file is provided");
        persisted.session = session.clone();
        ports.session_store.save_session(session_file, &persisted)?;
        Some(persisted)
    } else {
        None
    };

    Ok(ExtractCommandOutput {
        open: open_result,
        extract: extract_result,
        diagnostics: Some(diagnostics),
        session_state: persisted
            .as_ref()
            .map(|persisted| persisted.session.state.clone())
            .unwrap_or(session.state),
    })
}

pub(crate) fn handle_policy(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<PolicyCommandOutput, CliError> {
    let ports = ctx.ports;

    if options.session_file.is_some() {
        return Err(CliError::Usage(
            "Use `session-policy --session-file <path>` for persisted browser sessions."
                .to_string(),
        ));
    }

    if options.browser {
        let opened_at = current_timestamp();
        let browser_context_dir = options
            .session_file
            .as_ref()
            .map(|path| {
                ports
                    .session_store
                    .browser_context_dir_for_session(path.as_path())
            })
            .map(|path: std::path::PathBuf| path.display().to_string());
        let context = ports.browser.open_browser_session(
            &options.target,
            options.budget,
            options.source_risk.clone(),
            options.source_label.clone(),
            options.headed,
            browser_context_dir,
            None,
            "sclipolicy001",
            &opened_at,
        )?;
        let report = current_policy_with_allowlist(
            &context.session,
            ctx.policy_kernel,
            &options.allowlisted_domains,
        )
        .ok_or_else(|| CliError::Usage("Policy command requires an open snapshot.".to_string()))?;
        return Ok(PolicyCommandOutput {
            policy: report,
            session_state: context.session.state,
        });
    }

    if is_fixture_target(&options.target) {
        let catalog = ports.fixtures.load_catalog()?;
        let mut session = ctx
            .runtime
            .start_session("sclipolicy001", DEFAULT_OPENED_AT);
        ctx.runtime
            .open(&mut session, &catalog, &options.target, DEFAULT_OPENED_AT)?;
        let report = current_policy_with_allowlist(
            &session,
            ctx.policy_kernel,
            &options.allowlisted_domains,
        )
        .ok_or_else(|| CliError::Usage("Policy command requires an open snapshot.".to_string()))?;
        return Ok(PolicyCommandOutput {
            policy: report,
            session_state: session.state,
        });
    }

    let mut acquisition = ports.acquisition.create_engine()?;
    let opened_at = current_timestamp();
    let mut session = ctx.runtime.start_session("sclipolicy001", &opened_at);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.clone().unwrap_or(SourceRisk::Low),
        options.source_label.clone(),
        &opened_at,
    )?;
    if browser_fallback_reason(&snapshot).is_some() {
        return handle_policy(ctx, browser_fallback_target_options(&options));
    }
    let report =
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains)
            .ok_or_else(|| {
                CliError::Usage("Policy command requires an open snapshot.".to_string())
            })?;

    Ok(PolicyCommandOutput {
        policy: report,
        session_state: session.state,
    })
}

pub(crate) fn handle_replay(
    ctx: &CliAppContext<'_>,
    scenario: &str,
) -> Result<ReplayCommandOutput, CliError> {
    let catalog = ctx.ports.fixtures.load_catalog()?;
    let transcript_path = repo_root()
        .join("fixtures/scenarios")
        .join(scenario)
        .join("replay-transcript.json");
    let transcript: ReplayTranscript = serde_json::from_str(&fs::read_to_string(transcript_path)?)?;
    let session = ctx
        .runtime
        .replay(&catalog, &transcript, DEFAULT_OPENED_AT)?;

    Ok(ReplayCommandOutput {
        session_state: session.state,
        replay_transcript: session.transcript,
        snapshot_count: session.snapshots.len(),
        evidence_report_count: session.evidence_reports.len(),
    })
}

pub(crate) fn handle_memory_summary(
    ctx: &CliAppContext<'_>,
    steps: usize,
) -> Result<MemorySummaryOutput, CliError> {
    if steps == 0 || !steps.is_multiple_of(2) {
        return Err(CliError::Usage(
            "memory-summary requires an even `--steps` value greater than 0.".to_string(),
        ));
    }

    let catalog = ctx.ports.fixtures.load_catalog()?;
    let mut session = ctx
        .runtime
        .start_session("sclimemory001", DEFAULT_OPENED_AT);
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

        ctx.runtime
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

        ctx.runtime
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

    Ok(MemorySummaryOutput {
        requested_actions: steps,
        action_count: steps,
        session_state: session.state,
        memory_summary: summarize_turns(&memory_turns, 12),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        enrich_search_output_with_recovery, recovery_attempt_from_report, resolved_search_engine,
        search_retry_command, should_auto_recover_search, SearchCommandOutput, SearchEngine,
        SearchOptions, SearchReport, SearchReportStatus,
    };
    use crate::application::search_support::resolve_search_session_file;
    use touch_browser_contracts::{
        PolicyProfile, SessionMode, SessionState, SessionStatus, CONTRACT_VERSION,
    };

    fn test_search_report(engine: SearchEngine, status: SearchReportStatus) -> SearchReport {
        SearchReport {
            version: CONTRACT_VERSION.to_string(),
            generated_at: "2026-04-11T00:00:00+09:00".to_string(),
            engine,
            query: "cloudflare workers pricing".to_string(),
            search_url: format!(
                "https://{}.example/search?q=cloudflare+workers+pricing",
                match engine {
                    SearchEngine::Google => "google",
                    SearchEngine::Brave => "brave",
                }
            ),
            final_url: format!(
                "https://{}.example/final",
                match engine {
                    SearchEngine::Google => "google",
                    SearchEngine::Brave => "brave",
                }
            ),
            status,
            status_detail: (status == SearchReportStatus::Challenge)
                .then(|| "Challenge returned by provider.".to_string()),
            recovery: None,
            result_count: 0,
            results: Vec::new(),
            recommended_result_ranks: Vec::new(),
            next_action_hints: Vec::new(),
        }
    }

    fn test_session_state() -> SessionState {
        SessionState {
            version: CONTRACT_VERSION.to_string(),
            session_id: "sclisearch001".to_string(),
            mode: SessionMode::ReadOnly,
            status: SessionStatus::Active,
            policy_profile: PolicyProfile::ResearchReadOnly,
            current_url: Some("https://example.com".to_string()),
            opened_at: "2026-04-11T00:00:00+09:00".to_string(),
            updated_at: "2026-04-11T00:00:00+09:00".to_string(),
            visited_urls: vec!["https://example.com".to_string()],
            snapshot_ids: vec!["snap1".to_string()],
            working_set_refs: Vec::new(),
        }
    }

    fn test_output(
        engine: SearchEngine,
        status: SearchReportStatus,
        session_file: &str,
    ) -> SearchCommandOutput {
        let report = test_search_report(engine, status);
        SearchCommandOutput {
            query: "cloudflare workers pricing".to_string(),
            engine,
            search_url: report.search_url.clone(),
            result_count: report.result_count,
            search: report.clone(),
            result: report,
            browser_context_dir: None,
            browser_profile_dir: None,
            session_state: test_session_state(),
            session_file: session_file.to_string(),
        }
    }

    #[test]
    fn challenge_output_contains_structured_human_recovery_commands() {
        let output = test_output(
            SearchEngine::Google,
            SearchReportStatus::Challenge,
            "/tmp/google.search-session.json",
        );
        let attempts = vec![recovery_attempt_from_report(&output.result)];
        let enriched = enrich_search_output_with_recovery(output, &attempts);

        let recovery = enriched
            .result
            .recovery
            .expect("challenge output should include recovery trace");
        assert!(!recovery.recovered);
        assert!(recovery.human_intervention_required_now);
        assert_eq!(recovery.attempts.len(), 1);

        let complete = enriched
            .result
            .next_action_hints
            .iter()
            .find(|hint| hint.action == "complete-challenge")
            .expect("challenge hint should exist");
        assert_eq!(complete.engine, Some(SearchEngine::Google));
        assert_eq!(
            complete.actor,
            touch_browser_contracts::SearchActionActor::Human
        );
        assert!(complete.headed_required);
        assert!(complete
            .command
            .as_deref()
            .is_some_and(|command| command.contains("--engine google")));

        let resume = enriched
            .result
            .next_action_hints
            .iter()
            .find(|hint| hint.action == "resume-search")
            .expect("resume hint should exist");
        assert_eq!(resume.engine, Some(SearchEngine::Google));
        assert!(resume
            .command
            .as_deref()
            .is_some_and(|command| !command.contains("--headed")));
    }

    #[test]
    fn recovered_output_tracks_attempts_and_google_specific_followup() {
        let output = test_output(
            SearchEngine::Brave,
            SearchReportStatus::Ready,
            "/tmp/brave.search-session.json",
        );
        let attempts = vec![
            recovery_attempt_from_report(&test_search_report(
                SearchEngine::Google,
                SearchReportStatus::Challenge,
            )),
            recovery_attempt_from_report(&output.result),
        ];
        let enriched = enrich_search_output_with_recovery(output, &attempts);

        let recovery = enriched
            .result
            .recovery
            .expect("recovered output should include recovery trace");
        assert!(recovery.recovered);
        assert!(!recovery.human_intervention_required_now);
        assert_eq!(recovery.final_engine, SearchEngine::Brave);
        assert_eq!(recovery.attempts.len(), 2);

        let google_retry = enriched
            .result
            .next_action_hints
            .iter()
            .find(|hint| hint.action == "retry-google-headed")
            .expect("google retry hint should exist");
        assert_eq!(google_retry.engine, Some(SearchEngine::Google));
        assert!(google_retry.headed_required);
        assert!(google_retry
            .command
            .as_deref()
            .is_some_and(|command| command.contains("--engine google")));
    }

    #[test]
    fn default_search_session_hints_omit_session_file_flag() {
        let default_google_session = resolve_search_session_file(None, SearchEngine::Google)
            .display()
            .to_string();

        let command = search_retry_command(
            "cloudflare workers pricing",
            SearchEngine::Google,
            true,
            &default_google_session,
        );

        assert!(command.contains("--engine google"));
        assert!(command.contains("--headed"));
        assert!(!command.contains("--session-file"));
    }

    #[test]
    fn custom_search_session_hints_keep_session_file_flag() {
        let command = search_retry_command(
            "cloudflare workers pricing",
            SearchEngine::Google,
            true,
            "/tmp/custom-google-session.json",
        );

        assert!(command.contains("--engine google"));
        assert!(command.contains("--headed"));
        assert!(command.contains("--session-file"));
    }

    #[test]
    fn explicit_google_search_disables_brave_fallback_policy() {
        let options = SearchOptions {
            query: "cloudflare workers pricing".to_string(),
            engine: SearchEngine::Google,
            engine_explicit: true,
            budget: 600,
            headed: false,
            profile_dir: None,
            session_file: None,
        };

        assert!(
            !should_auto_recover_search(&options),
            "explicit google requests should stay pinned to google"
        );
        assert_eq!(
            resolved_search_engine(&options).expect("explicit engine should resolve"),
            SearchEngine::Google
        );
    }

    #[test]
    fn explicit_google_challenge_hints_never_point_to_brave() {
        let output = test_output(
            SearchEngine::Google,
            SearchReportStatus::Challenge,
            "/tmp/google.search-session.json",
        );
        let attempts = vec![recovery_attempt_from_report(&output.result)];
        let enriched = enrich_search_output_with_recovery(output, &attempts);

        assert!(enriched
            .result
            .next_action_hints
            .iter()
            .all(|hint| hint.engine != Some(SearchEngine::Brave)));
        assert!(enriched
            .result
            .next_action_hints
            .iter()
            .all(|hint| hint.action != "retry-google-headed"));
    }
}
