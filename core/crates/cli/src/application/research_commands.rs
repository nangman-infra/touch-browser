use std::fs;

use super::{
    context::CliAppContext,
    search_support::{
        build_search_report, build_search_url, default_search_session_file,
        derived_search_result_session_file, resolve_search_session_file,
        search_engine_source_label,
    },
};
use crate::{
    current_policy_with_allowlist, fail_action, is_fixture_target, plan_memory_turn, repo_root,
    slot_timestamp, succeed_action, summarize_turns, verify_action_result_if_requested,
    ActionCommand, ActionFailureKind, ActionName, ActionResult, ActionStatus, BrowserOrigin,
    ClaimInput, CliError, CompactSnapshotOutput, ExtractCommandOutput, ExtractOptions,
    MemorySummaryOutput, PolicyCommandOutput, ReadViewOutput, ReplayCommandOutput,
    ReplayTranscript, RiskClass, SearchCommandOutput, SearchEngine, SearchNextCommands,
    SearchOpenResultCommandOutput, SearchOpenResultOptions, SearchOpenTopCommandOutput,
    SearchOpenTopItem, SearchOpenTopOptions, SearchOptions, SearchReport, SearchReportStatus,
    SearchResultItem, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotDocument,
    SourceRisk, SourceType, TargetOptions, CONTRACT_VERSION, DEFAULT_OPENED_AT,
};
use serde::Serialize;

const JS_PLACEHOLDER_HINTS: &[&str] = &[
    "enable javascript",
    "requires javascript",
    "javascript to run this app",
    "turn javascript on",
    "javascript is disabled",
    "you need to enable javascript",
];

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

fn normalized_block_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn is_meaningful_snapshot_block(block: &SnapshotBlock) -> bool {
    let char_count = block.text.trim().chars().count();
    match block.kind {
        SnapshotBlockKind::Heading => char_count >= 4,
        SnapshotBlockKind::Text => char_count >= 32,
        SnapshotBlockKind::List | SnapshotBlockKind::Table => char_count >= 24,
        SnapshotBlockKind::Link => block.stable_ref.starts_with("rmain:") && char_count >= 48,
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Input => false,
    }
}

fn is_longform_content_block(block: &SnapshotBlock) -> bool {
    let char_count = block.text.trim().chars().count();
    match block.kind {
        SnapshotBlockKind::Heading => {
            !matches!(
                block.role,
                SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav
            ) && char_count >= 8
        }
        SnapshotBlockKind::Text => char_count >= 80,
        SnapshotBlockKind::List | SnapshotBlockKind::Table => char_count >= 64,
        SnapshotBlockKind::Link => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) && char_count >= 72
        }
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Input => false,
    }
}

fn is_shell_like_block(block: &SnapshotBlock) -> bool {
    if matches!(
        block.role,
        SnapshotBlockRole::PrimaryNav
            | SnapshotBlockRole::SecondaryNav
            | SnapshotBlockRole::Cta
            | SnapshotBlockRole::FormControl
    ) {
        return true;
    }

    match block.kind {
        SnapshotBlockKind::Link
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Input => true,
        SnapshotBlockKind::List => block.text.split_whitespace().count() <= 12,
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Heading
        | SnapshotBlockKind::Text
        | SnapshotBlockKind::Table => false,
    }
}

fn snapshot_requires_browser_fallback(snapshot: &SnapshotDocument) -> bool {
    if snapshot.source.source_type != SourceType::Http {
        return false;
    }

    if snapshot.blocks.is_empty() {
        return true;
    }

    let normalized_blocks = snapshot
        .blocks
        .iter()
        .map(|block| normalized_block_text(&block.text))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();

    if normalized_blocks
        .iter()
        .any(|text| JS_PLACEHOLDER_HINTS.iter().any(|hint| text.contains(hint)))
    {
        return true;
    }

    let meaningful_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| is_meaningful_snapshot_block(block))
        .collect::<Vec<_>>();
    let main_blocks = meaningful_blocks
        .iter()
        .filter(|block| block.stable_ref.starts_with("rmain:"))
        .count();
    let meaningful_chars = meaningful_blocks
        .iter()
        .map(|block| block.text.trim().chars().count())
        .sum::<usize>();

    if main_blocks == 0 && meaningful_blocks.len() <= 2 && meaningful_chars < 240 {
        return true;
    }

    let longform_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| is_longform_content_block(block))
        .count();
    let shell_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| is_shell_like_block(block))
        .count();
    let content_headings = snapshot
        .blocks
        .iter()
        .filter(|block| {
            matches!(block.kind, SnapshotBlockKind::Heading)
                && !matches!(
                    block.role,
                    SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav
                )
                && block.text.trim().chars().count() >= 4
        })
        .count();
    let text_like_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.kind,
                SnapshotBlockKind::Text | SnapshotBlockKind::List | SnapshotBlockKind::Table
            )
        })
        .count();

    (longform_blocks == 0 && text_like_blocks <= 1 && shell_blocks >= 8)
        || (longform_blocks <= 1
            && meaningful_chars < 320
            && shell_blocks >= 10
            && content_headings <= 1)
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

pub(crate) fn handle_search(
    ctx: &CliAppContext<'_>,
    options: SearchOptions,
) -> Result<SearchCommandOutput, CliError> {
    let ports = ctx.ports;
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
            ports
                .session_store
                .browser_context_dir_for_session(&session_file)
                .display()
                .to_string(),
        )
    };
    let context = ports.browser.open_browser_session(
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
                source_label: Some(search_engine_source_label(options.engine).to_string()),
            }),
            Vec::new(),
            Vec::new(),
            Some(report.clone()),
        ),
    )?;

    Ok(SearchCommandOutput {
        query: options.query,
        engine: options.engine,
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

    let context = ports.browser.open_browser_session(
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
                target: selected.url.clone(),
                source_risk: Some(SourceRisk::Low),
                source_label: None,
            }),
            persisted.allowlisted_domains.clone(),
            Vec::new(),
            Some(latest_search.clone()),
        ),
    )?;
    let opened = succeed_action(
        ActionName::Open,
        "snapshot-document",
        context.snapshot,
        "Opened browser-backed document.",
        current_policy_with_allowlist(
            &context.session,
            ctx.policy_kernel,
            &persisted.allowlisted_domains,
        ),
    )?;

    let session_extract_hint = if session_file == default_search_session_file(latest_search.engine)
    {
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
            let context = ports.browser.open_browser_session(
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
            ports.session_store.save_session(
                &result_session_file,
                &ports.browser.build_browser_cli_session(
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
            let opened = succeed_action(
                ActionName::Open,
                "snapshot-document",
                context.snapshot,
                "Opened browser-backed document.",
                current_policy_with_allowlist(
                    &context.session,
                    ctx.policy_kernel,
                    &persisted.allowlisted_domains,
                ),
            )?;

            Ok::<SearchOpenTopItem, CliError>(SearchOpenTopItem {
                rank: selected.rank,
                selected_result: selected,
                session_file: result_session_file.display().to_string(),
                result: opened,
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
    let mut session = ctx.runtime.start_session("scliopen001", DEFAULT_OPENED_AT);
    let source_risk = options.source_risk.clone().unwrap_or(SourceRisk::Low);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        source_risk.clone(),
        options.source_label.clone(),
        DEFAULT_OPENED_AT,
    )?;
    if snapshot_requires_browser_fallback(&snapshot) {
        return handle_browser_open(ctx, browser_fallback_target_options(&options));
    }
    let policy =
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains);

    succeed_action(
        ActionName::Open,
        "snapshot-document",
        snapshot,
        "Opened live document.",
        policy,
    )
}

pub(crate) fn handle_browser_open(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<ActionResult, CliError> {
    let ports = ctx.ports;
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
        DEFAULT_OPENED_AT,
    )?;
    if let Some(session_file) = options.session_file.as_ref() {
        ports.session_store.save_session(
            session_file,
            &ports.browser.build_browser_cli_session(
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

    succeed_action(
        ActionName::Open,
        "snapshot-document",
        context.snapshot,
        "Opened browser-backed document.",
        current_policy_with_allowlist(
            &context.session,
            ctx.policy_kernel,
            &options.allowlisted_domains,
        ),
    )
}

pub(crate) fn handle_compact_view(
    ctx: &CliAppContext<'_>,
    options: TargetOptions,
) -> Result<CompactSnapshotOutput, CliError> {
    let ports = ctx.ports;
    if target_requires_browser_session(&options) {
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
            DEFAULT_OPENED_AT,
        )?;

        if let Some(session_file) = options.session_file.as_ref() {
            ports.session_store.save_session(
                session_file,
                &ports.browser.build_browser_cli_session(
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

        return Ok(CompactSnapshotOutput::new(
            &context.snapshot,
            Some(context.session.state),
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
    let mut session = ctx
        .runtime
        .start_session("sclicompact001", DEFAULT_OPENED_AT);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.clone().unwrap_or(SourceRisk::Low),
        options.source_label.clone(),
        DEFAULT_OPENED_AT,
    )?;
    if snapshot_requires_browser_fallback(&snapshot) {
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
            DEFAULT_OPENED_AT,
        )?;

        if let Some(session_file) = options.session_file.as_ref() {
            ports.session_store.save_session(
                session_file,
                &ports.browser.build_browser_cli_session(
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

        return Ok(ReadViewOutput::new(
            &context.snapshot,
            Some(context.session.state),
            options.session_file.map(|path| path.display().to_string()),
            options.main_only,
        ));
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
    let mut session = ctx.runtime.start_session("scliread001", DEFAULT_OPENED_AT);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.clone().unwrap_or(SourceRisk::Low),
        options.source_label.clone(),
        DEFAULT_OPENED_AT,
    )?;
    if snapshot_requires_browser_fallback(&snapshot) {
        return handle_read_view(ctx, browser_fallback_target_options(&options));
    }

    Ok(ReadViewOutput::new(
        &snapshot,
        Some(session.state),
        None,
        options.main_only,
    ))
}

pub(crate) fn handle_extract(
    ctx: &CliAppContext<'_>,
    options: ExtractOptions,
) -> Result<ExtractCommandOutput, CliError> {
    let ports = ctx.ports;
    let claims = claim_inputs_from_statements(&options.claims)?;

    if extract_requires_browser_session(&options) {
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
            DEFAULT_OPENED_AT,
        )?;
        let open_result = succeed_action(
            ActionName::Open,
            "snapshot-document",
            context.snapshot.clone(),
            "Opened browser-backed document.",
            current_policy_with_allowlist(
                &context.session,
                ctx.policy_kernel,
                &options.allowlisted_domains,
            ),
        )?;
        let mut session = context.session;
        let extract_timestamp = slot_timestamp(1, 30);
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
        if let Some(session_file) = options.session_file.as_ref() {
            ports.session_store.save_session(
                session_file,
                &ports.browser.build_browser_cli_session(
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

        return Ok(ExtractCommandOutput {
            open: open_result,
            extract: extract_result,
            session_state: session.state,
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
            session_state: session.state,
        });
    }

    let mut acquisition = ports.acquisition.create_engine()?;
    let mut session = ctx
        .runtime
        .start_session("scliextract001", DEFAULT_OPENED_AT);
    let source_risk = options.source_risk.clone().unwrap_or(SourceRisk::Low);

    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        source_risk,
        options.source_label.clone(),
        DEFAULT_OPENED_AT,
    )?;
    if snapshot_requires_browser_fallback(&snapshot) {
        return handle_extract(ctx, browser_fallback_extract_options(&options));
    }
    let open_policy =
        current_policy_with_allowlist(&session, ctx.policy_kernel, &options.allowlisted_domains);
    let open_result = succeed_action(
        ActionName::Open,
        "snapshot-document",
        snapshot,
        "Opened live document.",
        open_policy.clone(),
    )?;

    let extract_timestamp = slot_timestamp(1, 30);
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
        session_state: session.state,
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
            DEFAULT_OPENED_AT,
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
    let mut session = ctx
        .runtime
        .start_session("sclipolicy001", DEFAULT_OPENED_AT);
    let snapshot = ctx.runtime.open_live(
        &mut session,
        &mut acquisition,
        &options.target,
        options.budget,
        options.source_risk.clone().unwrap_or(SourceRisk::Low),
        options.source_label.clone(),
        DEFAULT_OPENED_AT,
    )?;
    if snapshot_requires_browser_fallback(&snapshot) {
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
