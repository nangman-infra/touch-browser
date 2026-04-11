use serde::Serialize;
use serde_json::Value;

use super::deps::{
    application, infrastructure, ApproveOptions, CliCommand, CliError, ClickOptions, ExpandOptions,
    ExtractOptions, FollowOptions, PaginateOptions, SearchOpenResultOptions, SearchOpenTopOptions,
    SearchOptions, SessionExtractOptions, SessionFileOptions, SessionProfileSetOptions,
    SessionReadOptions, SessionRefreshOptions, SessionSynthesizeOptions, SubmitOptions,
    TargetOptions, TelemetryRecentOptions, TypeOptions, UninstallOptions, UpdateOptions,
};

fn serialize_output<T: Serialize>(output: T) -> Result<Value, CliError> {
    Ok(serde_json::to_value(output)?)
}

fn default_cli_ports() -> application::ports::CliPorts<'static> {
    application::ports::CliPorts {
        session_store: &infrastructure::app_ports::DEFAULT_SESSION_STORE,
        browser: &infrastructure::app_ports::DEFAULT_BROWSER_AUTOMATION,
        fixtures: &infrastructure::app_ports::DEFAULT_FIXTURE_CATALOG,
        acquisition: &infrastructure::app_ports::DEFAULT_ACQUISITION_FACTORY,
        verifier: &infrastructure::app_ports::DEFAULT_EVIDENCE_VERIFIER,
        telemetry: &infrastructure::app_ports::DEFAULT_TELEMETRY,
    }
}

fn default_app_context() -> application::context::CliAppContext<'static> {
    application::context::CliAppContext::new(
        default_cli_ports(),
        application::context::default_runtime(),
        application::context::default_action_vm(),
        application::context::default_policy_kernel(),
    )
}

pub(crate) fn dispatch(command: CliCommand) -> Result<Value, CliError> {
    let ctx = default_app_context();
    match command {
        CliCommand::Search(options) => handle_search(&ctx, options),
        CliCommand::SearchOpenResult(options) => handle_search_open_result(&ctx, options),
        CliCommand::SearchOpenTop(options) => handle_search_open_top(&ctx, options),
        CliCommand::Update(options) => handle_update(&ctx, options),
        CliCommand::Uninstall(options) => handle_uninstall(&ctx, options),
        CliCommand::Open(options) | CliCommand::Snapshot(options) => handle_open(&ctx, options),
        CliCommand::CompactView(options) => handle_compact_view(&ctx, options),
        CliCommand::ReadView(options) => handle_read_view(&ctx, options),
        CliCommand::Extract(options) => handle_extract(&ctx, options),
        CliCommand::Policy(options) => handle_policy(&ctx, options),
        CliCommand::SessionSnapshot(options) => handle_session_snapshot(&ctx, options),
        CliCommand::SessionCompact(options) => handle_session_compact(&ctx, options),
        CliCommand::SessionRead(options) => handle_session_read(&ctx, options),
        CliCommand::SessionRefresh(options) => handle_session_refresh(&ctx, options),
        CliCommand::SessionExtract(options) => handle_session_extract(&ctx, options),
        CliCommand::SessionCheckpoint(options) => handle_session_checkpoint(&ctx, options),
        CliCommand::SessionPolicy(options) => handle_session_policy(&ctx, options),
        CliCommand::SessionProfile(options) => handle_session_profile(&ctx, options),
        CliCommand::SetProfile(options) => handle_set_profile(&ctx, options),
        CliCommand::SessionSynthesize(options) => handle_session_synthesize(&ctx, options),
        CliCommand::Approve(options) => handle_approve(&ctx, options),
        CliCommand::Follow(options) => handle_follow(&ctx, options),
        CliCommand::Click(options) => handle_click(&ctx, options),
        CliCommand::Type(options) => handle_type(&ctx, options),
        CliCommand::Submit(options) => handle_submit(&ctx, options),
        CliCommand::Paginate(options) => handle_paginate(&ctx, options),
        CliCommand::Expand(options) => handle_expand(&ctx, options),
        CliCommand::BrowserReplay(options) => handle_browser_replay(&ctx, options),
        CliCommand::SessionClose(options) => handle_session_close(&ctx, options),
        CliCommand::TelemetrySummary => handle_telemetry_summary(&ctx),
        CliCommand::TelemetryRecent(options) => handle_telemetry_recent(&ctx, options),
        CliCommand::Replay { scenario } => handle_replay(&ctx, &scenario),
        CliCommand::MemorySummary { steps } => handle_memory_summary(&ctx, steps),
        CliCommand::Serve => Err(CliError::Usage(
            "serve is handled directly and should not be dispatched.".to_string(),
        )),
    }
}

pub(crate) fn run_serve() -> Result<(), CliError> {
    crate::interface::serve_runtime::handle_serve()
}

fn handle_search(
    ctx: &application::context::CliAppContext<'_>,
    options: SearchOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_search(ctx, options)?)
}

fn handle_search_open_result(
    ctx: &application::context::CliAppContext<'_>,
    options: SearchOpenResultOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_search_open_result(
        ctx, options,
    )?)
}

fn handle_search_open_top(
    ctx: &application::context::CliAppContext<'_>,
    options: SearchOpenTopOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_search_open_top(
        ctx, options,
    )?)
}

fn handle_update(
    _ctx: &application::context::CliAppContext<'_>,
    options: UpdateOptions,
) -> Result<Value, CliError> {
    let current_install = infrastructure::installation::require_managed_install_manifest()?;
    let release = infrastructure::installation::fetch_release_target(
        &current_install,
        options.version.as_deref(),
    )?;
    let update_available = release.version != current_install.version;

    let result = if options.check || !update_available {
        application::models::UpdateResultValue {
            current_version: current_install.version.clone(),
            target_version: release.version.clone(),
            update_available,
            checked_only: true,
            installed: false,
            release_url: release.html_url.clone(),
            asset_name: release.tarball_asset.name.clone(),
            command_link: current_install.command_link.clone(),
            managed_bundle_root: current_install.managed_bundle_root.clone(),
        }
    } else {
        let installed = infrastructure::installation::install_release(&current_install, &release)?;
        application::models::UpdateResultValue {
            current_version: current_install.version,
            target_version: installed.manifest.version.clone(),
            update_available: true,
            checked_only: false,
            installed: true,
            release_url: installed.release.html_url.clone(),
            asset_name: installed.release.tarball_asset.name.clone(),
            command_link: installed.manifest.command_link.clone(),
            managed_bundle_root: installed.manifest.managed_bundle_root.clone(),
        }
    };

    serialize_output(application::models::UpdateCommandOutput {
        current_version: result.current_version.clone(),
        target_version: result.target_version.clone(),
        update_available: result.update_available,
        checked_only: result.checked_only,
        installed: result.installed,
        release_url: result.release_url.clone(),
        asset_name: result.asset_name.clone(),
        command_link: result.command_link.clone(),
        managed_bundle_root: result.managed_bundle_root.clone(),
        result,
    })
}

fn handle_uninstall(
    _ctx: &application::context::CliAppContext<'_>,
    options: UninstallOptions,
) -> Result<Value, CliError> {
    if !options.yes {
        return Err(CliError::Usage(
            "uninstall is destructive. Re-run with `--yes` after reviewing the command."
                .to_string(),
        ));
    }

    let current_install = infrastructure::installation::require_managed_install_manifest()?;
    let uninstalled = infrastructure::installation::uninstall_managed_install(
        &current_install,
        options.purge_data,
        options.purge_all,
    )?;
    let result = application::models::UninstallResultValue {
        removed_paths: uninstalled.removed_paths.clone(),
        purged_data: options.purge_data,
        purged_all: options.purge_all,
    };

    serialize_output(application::models::UninstallCommandOutput {
        removed_paths: result.removed_paths.clone(),
        purged_data: result.purged_data,
        purged_all: result.purged_all,
        result,
    })
}

fn handle_open(
    ctx: &application::context::CliAppContext<'_>,
    options: TargetOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_open(ctx, options)?)
}

fn handle_compact_view(
    ctx: &application::context::CliAppContext<'_>,
    options: TargetOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_compact_view(
        ctx, options,
    )?)
}

fn handle_read_view(
    ctx: &application::context::CliAppContext<'_>,
    options: TargetOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_read_view(
        ctx, options,
    )?)
}

fn handle_extract(
    ctx: &application::context::CliAppContext<'_>,
    options: ExtractOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_extract(
        ctx, options,
    )?)
}

fn handle_policy(
    ctx: &application::context::CliAppContext<'_>,
    options: TargetOptions,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_policy(ctx, options)?)
}

fn handle_replay(
    ctx: &application::context::CliAppContext<'_>,
    scenario: &str,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_replay(
        ctx, scenario,
    )?)
}

fn handle_memory_summary(
    ctx: &application::context::CliAppContext<'_>,
    steps: usize,
) -> Result<Value, CliError> {
    serialize_output(application::research_commands::handle_memory_summary(
        ctx, steps,
    )?)
}

fn handle_session_snapshot(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_snapshot(
        ctx, options,
    )?)
}

fn handle_session_compact(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_compact(
        ctx, options,
    )?)
}

fn handle_session_read(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionReadOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_read(
        ctx, options,
    )?)
}

fn handle_session_refresh(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionRefreshOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_refresh(
        ctx, options,
    )?)
}

fn handle_session_extract(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionExtractOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_extract(
        ctx, options,
    )?)
}

fn handle_session_policy(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_policy(
        ctx, options,
    )?)
}

fn handle_session_profile(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_profile(
        ctx, options,
    )?)
}

fn handle_set_profile(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionProfileSetOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_set_profile(
        ctx, options,
    )?)
}

fn handle_session_checkpoint(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_checkpoint(
        ctx, options,
    )?)
}

fn handle_session_synthesize(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionSynthesizeOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_synthesize(
        ctx, options,
    )?)
}

fn handle_approve(
    ctx: &application::context::CliAppContext<'_>,
    options: ApproveOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_approve(ctx, options)?)
}

fn handle_telemetry_summary(
    ctx: &application::context::CliAppContext<'_>,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_telemetry_summary(
        ctx,
    )?)
}

fn handle_telemetry_recent(
    ctx: &application::context::CliAppContext<'_>,
    options: TelemetryRecentOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_telemetry_recent(
        ctx, options,
    )?)
}

fn handle_follow(
    ctx: &application::context::CliAppContext<'_>,
    options: FollowOptions,
) -> Result<Value, CliError> {
    serialize_output(application::browser_session_actions::handle_follow(
        ctx, options,
    )?)
}

fn handle_click(
    ctx: &application::context::CliAppContext<'_>,
    options: ClickOptions,
) -> Result<Value, CliError> {
    serialize_output(application::browser_session_actions::handle_click(
        ctx, options,
    )?)
}

fn handle_type(
    ctx: &application::context::CliAppContext<'_>,
    options: TypeOptions,
) -> Result<Value, CliError> {
    serialize_output(application::browser_session_actions::handle_type(
        ctx, options,
    )?)
}

fn handle_submit(
    ctx: &application::context::CliAppContext<'_>,
    options: SubmitOptions,
) -> Result<Value, CliError> {
    serialize_output(application::browser_session_actions::handle_submit(
        ctx, options,
    )?)
}

fn handle_paginate(
    ctx: &application::context::CliAppContext<'_>,
    options: PaginateOptions,
) -> Result<Value, CliError> {
    serialize_output(application::browser_session_actions::handle_paginate(
        ctx, options,
    )?)
}

fn handle_expand(
    ctx: &application::context::CliAppContext<'_>,
    options: ExpandOptions,
) -> Result<Value, CliError> {
    serialize_output(application::browser_session_actions::handle_expand(
        ctx, options,
    )?)
}

fn handle_session_close(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_session_close(
        ctx, options,
    )?)
}

fn handle_browser_replay(
    ctx: &application::context::CliAppContext<'_>,
    options: SessionFileOptions,
) -> Result<Value, CliError> {
    serialize_output(application::session_commands::handle_browser_replay(
        ctx, options,
    )?)
}
