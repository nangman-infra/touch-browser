use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use serde::Serialize;
use serde_json::{json, Value};

use super::agent_contract;
use super::cli_support::{data_root, node_executable, repo_root, resource_root};
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
        CliCommand::Capabilities => Ok(agent_contract::capabilities_payload()),
        CliCommand::Quickstart => Ok(agent_contract::quickstart_payload()),
        CliCommand::Doctor => handle_doctor(),
        CliCommand::Search(options) => handle_search(&ctx, options),
        CliCommand::SearchOpenResult(options) => handle_search_open_result(&ctx, options),
        CliCommand::SearchOpenTop(options) => handle_search_open_top(&ctx, options),
        CliCommand::Mcp => Err(CliError::Usage(
            "mcp is handled directly and should not be dispatched.".to_string(),
        )),
        CliCommand::Update(options) => handle_update(&ctx, options),
        CliCommand::Uninstall(options) => handle_uninstall(&ctx, options),
        CliCommand::Quick(options) => handle_quick(&ctx, options),
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

fn handle_doctor() -> Result<Value, CliError> {
    let current_exe = env::current_exe().ok();
    let repo_root = repo_root();
    let resource_root = resource_root();
    let data_root = data_root();
    let node = node_executable();
    let mcp_bridge = resolve_doctor_mcp_bridge(&resource_root, &repo_root);
    let node_version = command_stdout(&node, &["--version"], Some(&resource_root));
    let playwright_probe = probe_playwright_chromium(&node, &resource_root);

    let checks = vec![
        doctor_check(
            "current-executable",
            current_exe.as_ref().is_some_and(|path| path.is_file()),
            current_exe.as_ref().map(|path| path.display().to_string()),
            "touch-browser binary is resolvable.",
            "Could not resolve the running touch-browser executable.",
        ),
        doctor_check(
            "resource-root",
            resource_root.is_dir(),
            Some(resource_root.display().to_string()),
            "runtime resource root exists.",
            "Runtime resource root is missing.",
        ),
        doctor_check(
            "data-root-parent",
            data_root
                .parent()
                .is_some_and(|parent| parent.exists() && parent.is_dir()),
            Some(data_root.display().to_string()),
            "data root parent exists.",
            "Data root parent does not exist; create it or set TOUCH_BROWSER_DATA_ROOT.",
        ),
        doctor_check(
            "node",
            node_version.is_ok(),
            Some(node.clone()),
            node_version.as_deref().unwrap_or("node is runnable."),
            node_version
                .as_ref()
                .err()
                .map(String::as_str)
                .unwrap_or("node is not runnable."),
        ),
        doctor_check(
            "playwright-chromium",
            playwright_probe.ok,
            playwright_probe.path.clone(),
            playwright_probe
                .detail
                .as_deref()
                .unwrap_or("Playwright Chromium is installed."),
            playwright_probe
                .detail
                .as_deref()
                .unwrap_or("Playwright Chromium is not installed."),
        ),
        doctor_check(
            "mcp-bridge",
            mcp_bridge.as_ref().is_some_and(|path| path.is_file()),
            mcp_bridge.as_ref().map(|path| path.display().to_string()),
            "MCP bridge entrypoint exists.",
            "MCP bridge entrypoint is missing.",
        ),
        doctor_check(
            "semantic-model-paths",
            semantic_model_paths_ready(&resource_root),
            Some(resource_root.display().to_string()),
            "semantic model paths are available or lazy downloads are enabled.",
            "semantic model paths are missing and lazy downloads are disabled.",
        ),
    ];

    let failed = checks
        .iter()
        .filter(|check| check.get("status").and_then(Value::as_str) == Some("error"))
        .count();
    let status = if failed == 0 {
        "ready"
    } else {
        "attention-required"
    };

    Ok(json!({
        "status": status,
        "version": env!("CARGO_PKG_VERSION"),
        "contractVersion": touch_browser_contracts::CONTRACT_VERSION,
        "runtime": {
            "currentExecutable": current_exe.map(|path| path.display().to_string()),
            "repoRoot": repo_root.display().to_string(),
            "resourceRoot": resource_root.display().to_string(),
            "dataRoot": data_root.display().to_string(),
            "nodeExecutable": node,
            "mcpBridge": mcp_bridge.map(|path| path.display().to_string())
        },
        "checks": checks,
        "summary": {
            "failedChecks": failed,
            "ready": failed == 0
        },
        "nextActions": doctor_next_actions(failed)
    }))
}

fn doctor_check(
    name: &str,
    ok: bool,
    path: Option<String>,
    ok_detail: &str,
    error_detail: &str,
) -> Value {
    json!({
        "name": name,
        "status": if ok { "ok" } else { "error" },
        "path": path,
        "detail": if ok { ok_detail } else { error_detail }
    })
}

fn doctor_next_actions(failed: usize) -> Value {
    if failed == 0 {
        json!([
            {
                "action": "quick",
                "command": "touch-browser quick https://www.iana.org/help/example-domains --claim \"example.com is maintained for documentation purposes.\"",
                "actor": "ai",
                "canAutoRun": true,
                "headedRequired": false,
                "reason": "Run the first evidence proof path after preflight passes."
            }
        ])
    } else {
        json!([
            {
                "action": "repair-local-runtime",
                "command": "pnpm install && pnpm exec playwright install chromium",
                "actor": "human",
                "canAutoRun": false,
                "headedRequired": false,
                "reason": "Install missing local runtime dependencies before browser-backed use."
            }
        ])
    }
}

fn command_stdout(command: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut process = Command::new(command);
    process
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::piped());
    if let Some(cwd) = cwd {
        process.current_dir(cwd);
    }
    let output = process
        .output()
        .map_err(|error| format!("failed to run `{command}`: {error}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

struct PlaywrightProbe {
    ok: bool,
    path: Option<String>,
    detail: Option<String>,
}

fn probe_playwright_chromium(node: &str, cwd: &Path) -> PlaywrightProbe {
    let script = r#"
const { chromium } = require("playwright");
const path = chromium.executablePath();
console.log(JSON.stringify({ path }));
"#;
    match command_stdout(node, &["-e", script], Some(cwd)) {
        Ok(stdout) => {
            let path = serde_json::from_str::<Value>(&stdout)
                .ok()
                .and_then(|value| {
                    value
                        .get("path")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            let ok = path.as_ref().is_some_and(|path| Path::new(path).is_file());
            PlaywrightProbe {
                ok,
                path,
                detail: Some(if ok {
                    "Playwright Chromium executable exists.".to_string()
                } else {
                    "Playwright resolved Chromium, but the executable file is missing.".to_string()
                }),
            }
        }
        Err(error) => PlaywrightProbe {
            ok: false,
            path: None,
            detail: Some(error),
        },
    }
}

fn resolve_doctor_mcp_bridge(resource_root: &Path, repo_root: &Path) -> Option<PathBuf> {
    [
        resource_root.join("integrations/mcp/bridge/index.mjs"),
        resource_root.join("scripts/touch-browser-mcp-bridge.mjs"),
        repo_root.join("integrations/mcp/bridge/index.mjs"),
        repo_root.join("scripts/touch-browser-mcp-bridge.mjs"),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn semantic_model_paths_ready(resource_root: &Path) -> bool {
    lazy_download_enabled("TOUCH_BROWSER_EVIDENCE_EMBEDDING_LAZY_DOWNLOAD")
        && lazy_download_enabled("TOUCH_BROWSER_EVIDENCE_NLI_LAZY_DOWNLOAD")
        || model_path_ready(
            "TOUCH_BROWSER_EVIDENCE_EMBEDDING_MODEL_PATH",
            resource_root.join("models/evidence/embedding"),
        ) && model_path_ready(
            "TOUCH_BROWSER_EVIDENCE_NLI_MODEL_PATH",
            resource_root.join("models/evidence/nli"),
        )
}

fn lazy_download_enabled(name: &str) -> bool {
    env::var(name).map_or(true, |value| value != "0")
}

fn model_path_ready(env_name: &str, fallback: PathBuf) -> bool {
    env::var_os(env_name)
        .map(PathBuf::from)
        .unwrap_or(fallback)
        .is_dir()
}

pub(crate) fn run_serve() -> Result<(), CliError> {
    crate::interface::serve_runtime::handle_serve()
}

pub(crate) fn run_mcp() -> Result<(), CliError> {
    crate::interface::mcp_runtime::handle_mcp()
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

fn handle_quick(
    ctx: &application::context::CliAppContext<'_>,
    mut options: ExtractOptions,
) -> Result<Value, CliError> {
    options.browser = !super::cli_support::is_fixture_target(&options.target);
    handle_extract(ctx, options)
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
