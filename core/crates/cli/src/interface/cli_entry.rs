use serde_json::Value;

use crate::{
    dispatch, emit_cli_error, log_telemetry_error, log_telemetry_success, parse_command, run_serve,
    telemetry_surface_label, usage, CliCommand, CliError, OutputFormat,
};

#[derive(Debug, Clone, Default)]
pub(crate) struct ProcessedCliArgs {
    pub(crate) args: Vec<String>,
    pub(crate) json_errors: bool,
    pub(crate) help_text: Option<String>,
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

fn emit_read_view_quality_notice(output: &Value) {
    let quality = output.get("mainContentQuality").and_then(Value::as_str);
    let hint = output.get("mainContentHint").and_then(Value::as_str);
    if matches!(quality, Some("uncertain" | "poor")) {
        if let Some(hint) = hint {
            eprintln!("touch-browser note: {hint}");
        }
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

    ProcessedCliArgs {
        args,
        json_errors,
        help_text,
    }
}

pub(crate) fn command_usage(command_name: &str) -> Option<String> {
    let prefix = format!("  touch-browser {command_name} ");
    let usage_text = usage();
    let lines = usage_text
        .lines()
        .filter(|line| line.starts_with(&prefix))
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

    let args = processed_args.args;
    let json_errors = processed_args.json_errors;
    let operation = args
        .first()
        .cloned()
        .unwrap_or_else(|| "unknown".to_string());
    let command = match parse_command(&args) {
        Ok(command) => command,
        Err(error) => {
            let _ = log_telemetry_error(
                &telemetry_surface_label("cli"),
                &operation,
                &error.to_string(),
                None,
                &Value::Null,
            );
            emit_cli_error(&error, json_errors);
            return 1;
        }
    };

    if matches!(command, CliCommand::Serve) {
        if let Err(error) = run_serve() {
            emit_cli_error(&error, json_errors);
            return 1;
        }
        return 0;
    }

    let stdout_mode = stdout_mode_for_command(&command);
    match dispatch(command).and_then(|output| {
        match stdout_mode {
            CliStdoutMode::Json => println!(
                "{}",
                serde_json::to_string_pretty(&output).expect("cli output should serialize")
            ),
            CliStdoutMode::ReadMarkdown => {
                emit_read_view_quality_notice(&output);
                println!("{}", required_output_string(&output, "markdownText")?)
            }
            CliStdoutMode::SynthesisMarkdown => {
                println!("{}", required_output_string(&output, "markdown")?)
            }
        }
        Ok(output)
    }) {
        Ok(output) => {
            let _ = log_telemetry_success(
                &telemetry_surface_label("cli"),
                &operation,
                &output,
                &Value::Null,
            );
            0
        }
        Err(error) => {
            let _ = log_telemetry_error(
                &telemetry_surface_label("cli"),
                &operation,
                &error.to_string(),
                None,
                &Value::Null,
            );
            emit_cli_error(&error, json_errors);
            1
        }
    }
}
