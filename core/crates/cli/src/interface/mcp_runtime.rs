use std::{
    env,
    path::PathBuf,
    process::{Command, Stdio},
};

use super::{
    cli_error::CliError,
    cli_support::{node_executable, repo_root, resource_root},
};

pub(crate) fn handle_mcp() -> Result<(), CliError> {
    let bridge_entrypoint = resolve_mcp_bridge_entrypoint()?;
    let current_exe = env::current_exe()?;

    let status = Command::new(node_executable())
        .arg(&bridge_entrypoint)
        .env("TOUCH_BROWSER_RESOURCE_ROOT", resource_root())
        .env("TOUCH_BROWSER_SERVE_BINARY", &current_exe)
        .env("TOUCH_BROWSER_TELEMETRY_SURFACE", "mcp")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() {
        return Ok(());
    }

    Err(CliError::Adapter(format!(
        "MCP bridge exited with status {}.",
        format_exit_status(status)
    )))
}

fn resolve_mcp_bridge_entrypoint() -> Result<PathBuf, CliError> {
    let candidates = [
        resource_root().join("integrations/mcp/bridge/index.mjs"),
        resource_root().join("scripts/touch-browser-mcp-bridge.mjs"),
        repo_root().join("integrations/mcp/bridge/index.mjs"),
        repo_root().join("scripts/touch-browser-mcp-bridge.mjs"),
    ];

    candidates
        .into_iter()
        .find(|path| path.is_file())
        .ok_or_else(|| {
            CliError::Usage(
                "Could not resolve the MCP bridge entrypoint. Install a standalone bundle or run from a repository checkout that includes integrations/mcp/bridge/index.mjs.".to_string(),
            )
        })
}

fn format_exit_status(status: std::process::ExitStatus) -> String {
    status
        .code()
        .map(|code| code.to_string())
        .unwrap_or_else(|| "terminated by signal".to_string())
}
