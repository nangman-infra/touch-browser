use std::path::PathBuf;

use serde_json::Value;

use crate::interface::deps::{
    parse_ack_risk, parse_output_format, parse_source_risk, AckRisk, CliError, ExtractOptions,
    OutputFormat, SessionFileOptions, SessionSynthesizeOptions, TargetOptions,
    DEFAULT_REQUESTED_TOKENS,
};

pub(crate) fn json_target_options(params: &Value) -> Result<TargetOptions, CliError> {
    let headed = json_bool(params, "headed").unwrap_or(false);
    ensure_research_headed_allowed(headed, "runtime.open/readView/extract/policy/compactView")?;

    Ok(TargetOptions {
        target: required_json_string(params, "target")?,
        budget: json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS),
        source_risk: optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?,
        source_label: optional_json_string(params, "sourceLabel"),
        allowlisted_domains: json_string_array(params, "allowDomains")?,
        browser: json_bool(params, "browser").unwrap_or(false),
        headed,
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

    let headed = json_bool(params, "headed").unwrap_or(false);
    ensure_research_headed_allowed(headed, "runtime.open/readView/extract/policy/compactView")?;

    Ok(ExtractOptions {
        target: required_json_string(params, "target")?,
        budget: json_usize(params, "budget").unwrap_or(DEFAULT_REQUESTED_TOKENS),
        source_risk: optional_json_string(params, "sourceRisk")
            .map(|value| parse_source_risk(&value))
            .transpose()?,
        source_label: optional_json_string(params, "sourceLabel"),
        allowlisted_domains: json_string_array(params, "allowDomains")?,
        browser: json_bool(params, "browser").unwrap_or(false),
        headed,
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

pub(crate) fn ensure_research_headed_allowed(
    headed_requested: bool,
    operation: &str,
) -> Result<(), CliError> {
    if !headed_requested {
        return Ok(());
    }

    Err(CliError::Usage(format!(
        "serve/MCP headed mode is restricted for `{operation}`. Keep browsing, search, open, read-view, and refresh flows headless. Headed mode is reserved for supervised challenge/auth/MFA recovery interactions."
    )))
}

pub(crate) fn ensure_recovery_headed_allowed(
    headed_requested: bool,
    operation: &str,
    ack_risks: &[AckRisk],
) -> Result<(), CliError> {
    if !headed_requested {
        return Ok(());
    }

    if ack_risks
        .iter()
        .any(|risk| matches!(risk, AckRisk::Challenge | AckRisk::Mfa | AckRisk::Auth))
    {
        return Ok(());
    }

    Err(CliError::Usage(format!(
        "serve/MCP headed mode is restricted for `{operation}`. Headed recovery is allowed only for challenge/auth/MFA handling with explicit ackRisks."
    )))
}

#[cfg(test)]
mod tests {
    use super::{ensure_recovery_headed_allowed, ensure_research_headed_allowed};
    use crate::interface::deps::AckRisk;

    #[test]
    fn research_headed_is_rejected() {
        let error = ensure_research_headed_allowed(true, "runtime.search")
            .expect_err("research headed should be rejected");
        assert!(error
            .to_string()
            .contains("serve/MCP headed mode is restricted"));
    }

    #[test]
    fn recovery_headed_requires_explicit_recovery_risk() {
        let error = ensure_recovery_headed_allowed(true, "runtime.session.click", &[])
            .expect_err("headed recovery should require ack risks");
        assert!(error
            .to_string()
            .contains("allowed only for challenge/auth/MFA"));
    }

    #[test]
    fn recovery_headed_allows_auth_ack() {
        ensure_recovery_headed_allowed(true, "runtime.session.click", &[AckRisk::Auth])
            .expect("auth ack should allow headed recovery");
    }
}
