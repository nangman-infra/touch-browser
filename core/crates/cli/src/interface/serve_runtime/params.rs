use std::path::PathBuf;

use serde_json::Value;

use crate::{
    parse_ack_risk, parse_output_format, parse_source_risk, AckRisk, CliError, ExtractOptions,
    OutputFormat, SessionFileOptions, SessionSynthesizeOptions, TargetOptions,
    DEFAULT_REQUESTED_TOKENS,
};

pub(crate) fn json_target_options(params: &Value) -> Result<TargetOptions, CliError> {
    Ok(TargetOptions {
        target: required_json_string(params, "target")?,
        budget: json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS),
        source_risk: optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?,
        source_label: optional_json_string(params, "sourceLabel"),
        allowlisted_domains: json_string_array(params, "allowDomains")?,
        browser: json_bool(params, "browser").unwrap_or(false),
        headed: json_bool(params, "headed").unwrap_or(false),
        main_only: json_bool(params, "mainOnly").unwrap_or(false),
        session_file: optional_json_string(params, "sessionFile").map(PathBuf::from),
    })
}

pub(crate) fn json_extract_options(params: &Value) -> Result<ExtractOptions, CliError> {
    let claims = json_string_array(params, "claims")?;
    if claims.is_empty() {
        return Err(CliError::Usage(
            "serve params `claims` must include at least one statement.".to_string(),
        ));
    }

    Ok(ExtractOptions {
        target: required_json_string(params, "target")?,
        budget: json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS),
        source_risk: optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?,
        source_label: optional_json_string(params, "sourceLabel"),
        allowlisted_domains: json_string_array(params, "allowDomains")?,
        browser: json_bool(params, "browser").unwrap_or(false),
        headed: json_bool(params, "headed").unwrap_or(false),
        session_file: optional_json_string(params, "sessionFile").map(PathBuf::from),
        claims,
        verifier_command: optional_json_string(params, "verifierCommand"),
    })
}

pub(crate) fn json_session_file_options(params: &Value) -> Result<SessionFileOptions, CliError> {
    Ok(SessionFileOptions {
        session_file: PathBuf::from(required_json_string(params, "sessionFile")?),
    })
}

pub(crate) fn json_session_synthesize_options(
    params: &Value,
) -> Result<SessionSynthesizeOptions, CliError> {
    Ok(SessionSynthesizeOptions {
        session_file: PathBuf::from(required_json_string(params, "sessionFile")?),
        note_limit: json_usize(params, "noteLimit").unwrap_or(12),
        format: optional_json_string(params, "format")
            .map(|value| parse_output_format(&value))
            .transpose()?
            .unwrap_or(OutputFormat::Json),
    })
}

pub(crate) fn required_json_string(params: &Value, field: &str) -> Result<String, CliError> {
    optional_json_string(params, field)
        .ok_or_else(|| CliError::Usage(format!("serve params require `{field}` as a string.")))
}

pub(crate) fn optional_json_string(params: &Value, field: &str) -> Option<String> {
    params
        .get(field)
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub(crate) fn json_string_array(params: &Value, field: &str) -> Result<Vec<String>, CliError> {
    match params.get(field) {
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str().map(ToString::to_string).ok_or_else(|| {
                    CliError::Usage(format!(
                        "serve params `{field}` must be an array of strings."
                    ))
                })
            })
            .collect(),
        Some(_) => Err(CliError::Usage(format!(
            "serve params `{field}` must be an array of strings."
        ))),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn json_ack_risks(params: &Value, field: &str) -> Result<Vec<AckRisk>, CliError> {
    json_string_array(params, field)?
        .into_iter()
        .map(|value| parse_ack_risk(&value))
        .collect()
}

pub(crate) fn json_bool(params: &Value, field: &str) -> Option<bool> {
    params.get(field).and_then(Value::as_bool)
}

pub(crate) fn json_usize(params: &Value, field: &str) -> Option<usize> {
    params
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}
