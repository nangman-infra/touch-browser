use serde::Serialize;
use thiserror::Error;
use touch_browser_acquisition::AcquisitionError;
use touch_browser_observation::ObservationError;
use touch_browser_runtime::RuntimeError;
use touch_browser_storage_sqlite::TelemetryError;

#[derive(Debug, Serialize)]
pub(crate) struct CliErrorPayload {
    pub(crate) error: String,
    pub(crate) kind: String,
    pub(crate) message: String,
    pub(crate) hint: Option<String>,
}

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("{0}")]
    Usage(String),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("I/O error at {path}: {source}")]
    IoPath {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("JSON error in {path}: {source}")]
    JsonPath {
        path: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("observation error: {0}")]
    Observation(#[from] ObservationError),
    #[error("runtime error: {0}")]
    Runtime(#[from] RuntimeError),
    #[error("acquisition error: {0}")]
    Acquisition(#[from] AcquisitionError),
    #[error("telemetry error: {0}")]
    Telemetry(#[from] TelemetryError),
    #[error("network error: {0}")]
    Network(#[from] reqwest::Error),
    #[error("adapter error: {0}")]
    Adapter(String),
    #[error("verifier error: {0}")]
    Verifier(String),
}

pub(crate) fn emit_cli_error(error: &CliError, json_errors: bool) {
    if json_errors {
        let payload = build_cli_error_payload(error);
        let serialized = serde_json::to_string(&payload)
            .unwrap_or_else(|_| "{\"error\":\"serialization-failed\",\"kind\":\"internal-error\",\"message\":\"failed to serialize CLI error payload\",\"hint\":null}".to_string());
        eprintln!("{serialized}");
    } else {
        eprintln!("{error}");
    }
}

pub(crate) fn build_cli_error_payload(error: &CliError) -> CliErrorPayload {
    match error {
        CliError::Usage(message) => {
            let (code, hint) = usage_error_details(message);
            CliErrorPayload {
                error: code.to_string(),
                kind: "usage-error".to_string(),
                message: message.clone(),
                hint,
            }
        }
        CliError::Acquisition(_) => CliErrorPayload {
            error: "acquisition-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Observation(_) => CliErrorPayload {
            error: "observation-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Runtime(_) => CliErrorPayload {
            error: "runtime-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Adapter(_) => CliErrorPayload {
            error: "adapter-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Verifier(_) => CliErrorPayload {
            error: "verifier-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Telemetry(_) => CliErrorPayload {
            error: "telemetry-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Network(_) => CliErrorPayload {
            error: "network-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Io(_) | CliError::IoPath { .. } => CliErrorPayload {
            error: "io-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
        CliError::Json(_) | CliError::JsonPath { .. } => CliErrorPayload {
            error: "json-error".to_string(),
            kind: "runtime-error".to_string(),
            message: error.to_string(),
            hint: None,
        },
    }
}

fn usage_error_details(message: &str) -> (&'static str, Option<String>) {
    if message.contains("Unknown command") {
        return (
            "unknown-command",
            Some("run `touch-browser --help` to list supported commands.".to_string()),
        );
    }
    if message.contains("A target URL or fixture URI is required.") {
        return (
            "missing-target",
            Some(
                "provide a target URL or fixture URI as the first positional argument.".to_string(),
            ),
        );
    }
    if message.contains("--claim requires")
        || message.contains("requires `--claim")
        || message.contains("at least one `--claim`")
    {
        return (
            "missing-claim",
            Some("provide --claim <statement> at least once.".to_string()),
        );
    }
    if message.contains("--session-file requires")
        || message.contains("requires `--session-file <path>`")
    {
        return (
            "missing-session-file",
            Some("provide --session-file <path>.".to_string()),
        );
    }
    if message.contains("--ref requires") || message.contains("requires `--ref <stable-ref>`") {
        return (
            "missing-ref",
            Some("provide --ref <stable-ref>.".to_string()),
        );
    }
    if message.contains("--value requires") || message.contains("requires `--value <text>`") {
        return ("missing-value", Some("provide --value <text>.".to_string()));
    }
    if message.contains("--risk requires") || message.contains("--ack-risk requires") {
        return (
            "missing-risk",
            Some("provide the required risk acknowledgement value.".to_string()),
        );
    }
    if message.contains("uninstall is destructive") {
        return (
            "confirmation-required",
            Some("re-run uninstall with --yes after reviewing the command.".to_string()),
        );
    }
    ("usage-error", None)
}
