use serde_json::Value;

use super::deps::{
    dispatch, emit_cli_error, log_telemetry_error, log_telemetry_success, parse_command, run_mcp,
    run_serve, telemetry_surface_label, usage, CliCommand, CliError, OutputFormat,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ProcessedCliArgs {
    pub(crate) args: Vec<String>,
    pub(crate) json_errors: bool,
    pub(crate) help_text: Option<String>,
    pub(crate) version_text: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CliStdoutMode {
    Json,
    ReadMarkdown,
    SynthesisMarkdown,
}

fn is_help_flag(value: &str) -> bool {
    matches!(value, "--help" | "-h")
}

fn is_version_flag(value: &str) -> bool {
    matches!(value, "--version" | "-V")
}

fn version_text() -> String {
    format!("touch-browser {}", env!("CARGO_PKG_VERSION"))
}

fn emit_read_view_quality_notice(output: &Value) {
    let quality = output.get("mainContentQuality").and_then(Value::as_str);
    let reason = output.get("mainContentReason").and_then(Value::as_str);
    let hint = output.get("mainContentHint").and_then(Value::as_str);
    if matches!(quality, Some("uncertain" | "poor")) {
        if let Some(hint) = hint {
            match reason {
                Some(reason) => eprintln!("touch-browser note [{reason}]: {hint}"),
                None => eprintln!("touch-browser note: {hint}"),
            }
        }
    }
}

fn should_log_telemetry_operation(operation: &str) -> bool {
    operation != "uninstall"
}

fn should_log_telemetry_command(command: &CliCommand) -> bool {
    !matches!(command, CliCommand::Uninstall(_))
}

fn parse_command_or_exit(
    args: &[String],
    operation: &str,
    json_errors: bool,
) -> Result<CliCommand, i32> {
    parse_command(args).map_err(|error| {
        if should_log_telemetry_operation(operation) {
            let _ = log_telemetry_error(
                &telemetry_surface_label("cli"),
                operation,
                &error.to_string(),
                None,
                &Value::Null,
            );
        }
        emit_cli_error(&error, json_errors);
        1
    })
}

fn run_direct_command(command: &CliCommand, json_errors: bool) -> Option<i32> {
    let result = match command {
        CliCommand::Serve => Some(run_serve()),
        CliCommand::Mcp => Some(run_mcp()),
        _ => None,
    }?;

    Some(match result {
        Ok(()) => 0,
        Err(error) => {
            emit_cli_error(&error, json_errors);
            1
        }
    })
}

fn emit_command_output(stdout_mode: CliStdoutMode, output: &Value) -> Result<(), CliError> {
    match stdout_mode {
        CliStdoutMode::Json => println!(
            "{}",
            serde_json::to_string_pretty(output).expect("cli output should serialize")
        ),
        CliStdoutMode::ReadMarkdown => {
            emit_read_view_quality_notice(output);
            println!("{}", required_output_string(output, "markdownText")?)
        }
        CliStdoutMode::SynthesisMarkdown => {
            println!("{}", required_output_string(output, "markdown")?)
        }
    }
    Ok(())
}

fn emit_command_failure(
    operation: &str,
    error: &CliError,
    json_errors: bool,
    should_log_telemetry: bool,
) -> i32 {
    if should_log_telemetry {
        let _ = log_telemetry_error(
            &telemetry_surface_label("cli"),
            operation,
            &error.to_string(),
            None,
            &Value::Null,
        );
    }
    emit_cli_error(error, json_errors);
    1
}

fn dispatch_command(command: CliCommand, operation: &str, json_errors: bool) -> i32 {
    let stdout_mode = stdout_mode_for_command(&command);
    let should_log_telemetry = should_log_telemetry_command(&command);
    match dispatch(command).and_then(|output| {
        emit_command_output(stdout_mode, &output)?;
        Ok(output)
    }) {
        Ok(output) => {
            if should_log_telemetry {
                let _ = log_telemetry_success(
                    &telemetry_surface_label("cli"),
                    operation,
                    &output,
                    &Value::Null,
                );
            }
            0
        }
        Err(error) => emit_command_failure(operation, &error, json_errors, should_log_telemetry),
    }
}

pub(crate) fn preprocess_cli_args(raw_args: Vec<String>) -> ProcessedCliArgs {
    let mut json_errors = false;
    let mut args = Vec::with_capacity(raw_args.len());
    for arg in raw_args {
        if arg == "--json-errors" {
            json_errors = true;
        } else {
            args.push(arg);
        }
    }

    let help_text = if args.is_empty() {
        Some(usage())
    } else if matches!(args.first().map(String::as_str), Some("help")) {
        args.get(1)
            .and_then(|command| command_usage(command))
            .or_else(|| Some(usage()))
    } else if matches!(args.first().map(String::as_str), Some(flag) if is_help_flag(flag)) {
        Some(usage())
    } else if args.len() >= 2 && is_help_flag(&args[1]) {
        command_usage(&args[0])
    } else {
        None
    };

    let version_text = if args.len() == 1 && is_version_flag(&args[0]) {
        Some(version_text())
    } else {
        None
    };

    ProcessedCliArgs {
        args,
        json_errors,
        help_text,
        version_text,
    }
}

pub(crate) fn command_usage(command_name: &str) -> Option<String> {
    let exact = format!("  touch-browser {command_name}");
    let prefix = format!("{exact} ");
    let usage_text = usage();
    let lines = usage_text
        .lines()
        .filter(|line| *line == exact || line.starts_with(&prefix))
        .collect::<Vec<_>>();
    if lines.is_empty() {
        None
    } else {
        Some(format!("Usage:\n{}", lines.join("\n")))
    }
}

pub(crate) fn stdout_mode_for_command(command: &CliCommand) -> CliStdoutMode {
    match command {
        CliCommand::ReadView(_) | CliCommand::SessionRead(_) => CliStdoutMode::ReadMarkdown,
        CliCommand::SessionSynthesize(options) if options.format == OutputFormat::Markdown => {
            CliStdoutMode::SynthesisMarkdown
        }
        _ => CliStdoutMode::Json,
    }
}

pub(crate) fn required_output_string<'a>(
    output: &'a Value,
    field: &str,
) -> Result<&'a str, CliError> {
    output.get(field).and_then(Value::as_str).ok_or_else(|| {
        CliError::Usage(format!(
            "Expected `{field}` string output in CLI response payload."
        ))
    })
}

pub(crate) fn run_cli(raw_args: Vec<String>) -> i32 {
    let processed_args = preprocess_cli_args(raw_args);
    if let Some(help_text) = processed_args.help_text.as_deref() {
        println!("{help_text}");
        return 0;
    }
    if let Some(version_text) = processed_args.version_text.as_deref() {
        println!("{version_text}");
        return 0;
    }

    let args = processed_args.args;
    let json_errors = processed_args.json_errors;
    let operation = args
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let command = match parse_command_or_exit(&args, &operation, json_errors) {
        Ok(command) => command,
        Err(exit_code) => return exit_code,
    };

    if let Some(exit_code) = run_direct_command(&command, json_errors) {
        return exit_code;
    }

    dispatch_command(command, &operation, json_errors)
}

#[cfg(test)]
mod tests {
    use super::{
        command_usage, should_log_telemetry_command, should_log_telemetry_operation, version_text,
    };
    use crate::{CliCommand, SearchEngine, SearchOptions, UninstallOptions, DEFAULT_SEARCH_TOKENS};

    #[test]
    fn telemetry_logging_skips_uninstall_lifecycle() {
        assert!(!should_log_telemetry_operation("uninstall"));
        assert!(!should_log_telemetry_command(&CliCommand::Uninstall(
            UninstallOptions {
                purge_data: true,
                purge_all: true,
                yes: true,
            }
        )));
        assert!(should_log_telemetry_command(&CliCommand::Search(
            SearchOptions {
                query: "iana example domains".to_string(),
                engine: SearchEngine::Brave,
                engine_explicit: true,
                budget: DEFAULT_SEARCH_TOKENS,
                headed: false,
                profile_dir: None,
                session_file: None,
            }
        )));
    }

    #[test]
    fn command_usage_supports_commands_without_positional_arguments() {
        let serve_usage = command_usage("serve").expect("serve usage should exist");
        let mcp_usage = command_usage("mcp").expect("mcp usage should exist");

        assert_eq!(serve_usage, "Usage:\n  touch-browser serve");
        assert_eq!(mcp_usage, "Usage:\n  touch-browser mcp");
    }

    #[test]
    fn version_text_matches_package_version() {
        assert_eq!(
            version_text(),
            format!("touch-browser {}", env!("CARGO_PKG_VERSION"))
        );
    }
}
