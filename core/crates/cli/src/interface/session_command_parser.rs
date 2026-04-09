use std::path::PathBuf;

use crate::{
    AckRisk, ApproveOptions, CliError, ExtractOptions, OutputFormat, PolicyProfile,
    SessionExtractOptions, SessionFileOptions, SessionProfileSetOptions, SessionReadOptions,
    SessionRefreshOptions, SessionSynthesizeOptions, SourceRisk, TargetOptions,
    TelemetryRecentOptions, DEFAULT_REQUESTED_TOKENS,
};

use super::search_command_parser::parse_search_engine;

fn parse_claim_value(args: &[String], index: usize) -> Result<String, CliError> {
    let value = args
        .get(index + 1)
        .ok_or_else(|| CliError::Usage("--claim requires a statement.".to_string()))?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(CliError::Usage(
            "--claim requires a non-empty statement.".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn parse_target_options(args: &[String]) -> Result<TargetOptions, CliError> {
    let mut target = None;
    let mut options = TargetOptions {
        target: String::new(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: false,
        session_file: None,
    };
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--browser" => {
                options.browser = true;
                index += 1;
            }
            "--headed" => {
                options.headed = true;
                index += 1;
            }
            "--main-only" => {
                options.main_only = true;
                index += 1;
            }
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                options.session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--source-risk" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--source-risk requires a value.".to_string())
                })?;
                options.source_risk = Some(parse_source_risk(value)?);
                index += 2;
            }
            "--source-label" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--source-label requires a value.".to_string())
                })?;
                options.source_label = Some(value.clone());
                index += 2;
            }
            "--allow-domain" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--allow-domain requires a hostname.".to_string())
                })?;
                options.allowlisted_domains.push(value.clone());
                index += 2;
            }
            "--budget" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--budget requires a positive integer.".to_string())
                })?;
                options.budget = value.parse().map_err(|_| {
                    CliError::Usage("--budget requires a positive integer.".to_string())
                })?;
                if options.budget == 0 {
                    return Err(CliError::Usage(
                        "--budget requires a positive integer.".to_string(),
                    ));
                }
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for target command."
                )));
            }
            other => {
                if target.is_some() {
                    return Err(CliError::Usage(format!(
                        "Unexpected extra target argument `{other}`."
                    )));
                }
                target = Some(other.to_string());
                index += 1;
            }
        }
    }

    options.target = target
        .ok_or_else(|| CliError::Usage("A target URL or fixture URI is required.".to_string()))?;

    Ok(options)
}

pub(crate) fn parse_extract_options(args: &[String]) -> Result<ExtractOptions, CliError> {
    let mut target = None;
    let mut claims = Vec::new();
    let mut index = 0;
    let mut budget = DEFAULT_REQUESTED_TOKENS;
    let mut source_risk = None;
    let mut source_label = None;
    let mut allowlisted_domains = Vec::new();
    let mut browser = false;
    let mut headed = false;
    let mut session_file = None;
    let mut verifier_command = None;

    while index < args.len() {
        match args[index].as_str() {
            "--browser" => {
                browser = true;
                index += 1;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--source-risk" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--source-risk requires a value.".to_string())
                })?;
                source_risk = Some(parse_source_risk(value)?);
                index += 2;
            }
            "--source-label" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--source-label requires a value.".to_string())
                })?;
                source_label = Some(value.clone());
                index += 2;
            }
            "--allow-domain" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--allow-domain requires a hostname.".to_string())
                })?;
                allowlisted_domains.push(value.clone());
                index += 2;
            }
            "--budget" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--budget requires a positive integer.".to_string())
                })?;
                budget = value.parse().map_err(|_| {
                    CliError::Usage("--budget requires a positive integer.".to_string())
                })?;
                if budget == 0 {
                    return Err(CliError::Usage(
                        "--budget requires a positive integer.".to_string(),
                    ));
                }
                index += 2;
            }
            "--claim" => {
                claims.push(parse_claim_value(args, index)?);
                index += 2;
            }
            "--verifier-command" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--verifier-command requires a shell command.".to_string())
                })?;
                verifier_command = Some(value.clone());
                index += 2;
            }
            other if other.starts_with("--") => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for extract command."
                )));
            }
            other => {
                if target.is_some() {
                    return Err(CliError::Usage(format!(
                        "Unexpected extra target argument `{other}`."
                    )));
                }
                target = Some(other.to_string());
                index += 1;
            }
        }
    }

    if claims.is_empty() {
        return Err(CliError::Usage(
            "extract requires at least one `--claim` statement.".to_string(),
        ));
    }

    Ok(ExtractOptions {
        target: target.ok_or_else(|| {
            CliError::Usage("A target URL or fixture URI is required.".to_string())
        })?,
        budget,
        source_risk,
        source_label,
        allowlisted_domains,
        browser,
        headed,
        session_file,
        claims,
        verifier_command,
    })
}

pub(crate) fn parse_session_file_options(
    args: &[String],
    command_name: &str,
) -> Result<SessionFileOptions, CliError> {
    if args.len() == 2 && args[0] == "--session-file" {
        return Ok(SessionFileOptions {
            session_file: PathBuf::from(&args[1]),
        });
    }

    Err(CliError::Usage(format!(
        "{command_name} requires `--session-file <path>`."
    )))
}

pub(crate) fn parse_session_read_options(args: &[String]) -> Result<SessionReadOptions, CliError> {
    let mut session_file = None;
    let mut main_only = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--main-only" => {
                main_only = true;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for session-read command."
                )));
            }
        }
    }

    Ok(SessionReadOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("session-read requires `--session-file <path>`.".to_string())
        })?,
        main_only,
    })
}

pub(crate) fn parse_session_refresh_options(
    args: &[String],
) -> Result<SessionRefreshOptions, CliError> {
    let mut session_file = None;
    let mut headed = false;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for refresh command."
                )));
            }
        }
    }

    Ok(SessionRefreshOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("refresh requires `--session-file <path>`.".to_string())
        })?,
        headed,
    })
}

pub(crate) fn parse_ack_risk(value: &str) -> Result<AckRisk, CliError> {
    match value {
        "challenge" => Ok(AckRisk::Challenge),
        "mfa" => Ok(AckRisk::Mfa),
        "auth" => Ok(AckRisk::Auth),
        "high-risk-write" => Ok(AckRisk::HighRiskWrite),
        _ => Err(CliError::Usage(format!(
            "Unsupported `--ack-risk` value `{value}`. Expected one of: challenge, mfa, auth, high-risk-write."
        ))),
    }
}

pub(crate) fn parse_approve_options(args: &[String]) -> Result<ApproveOptions, CliError> {
    let mut session_file = None;
    let mut ack_risks = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--risk" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--risk requires a value.".to_string()))?;
                ack_risks.push(parse_ack_risk(value)?);
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for approve command."
                )));
            }
        }
    }

    if ack_risks.is_empty() {
        return Err(CliError::Usage(
            "approve requires at least one `--risk <value>`.".to_string(),
        ));
    }

    Ok(ApproveOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("approve requires `--session-file <path>`.".to_string())
        })?,
        ack_risks,
    })
}

pub(crate) fn parse_policy_profile(value: &str) -> Result<PolicyProfile, CliError> {
    match value {
        "research-read-only" => Ok(PolicyProfile::ResearchReadOnly),
        "research-restricted" => Ok(PolicyProfile::ResearchRestricted),
        "interactive-review" => Ok(PolicyProfile::InteractiveReview),
        "interactive-supervised-auth" => Ok(PolicyProfile::InteractiveSupervisedAuth),
        "interactive-supervised-write" => Ok(PolicyProfile::InteractiveSupervisedWrite),
        _ => Err(CliError::Usage(format!(
            "Unsupported `--profile` value `{value}`. Expected one of: research-read-only, research-restricted, interactive-review, interactive-supervised-auth, interactive-supervised-write."
        ))),
    }
}

pub(crate) fn parse_set_profile_options(
    args: &[String],
) -> Result<SessionProfileSetOptions, CliError> {
    let mut session_file = None;
    let mut profile = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--profile" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--profile requires a value.".to_string()))?;
                profile = Some(parse_policy_profile(value)?);
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for set-profile command."
                )));
            }
        }
    }

    Ok(SessionProfileSetOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("set-profile requires `--session-file <path>`.".to_string())
        })?,
        profile: profile.ok_or_else(|| {
            CliError::Usage("set-profile requires `--profile <value>`.".to_string())
        })?,
    })
}

pub(crate) fn parse_telemetry_recent_options(
    args: &[String],
) -> Result<TelemetryRecentOptions, CliError> {
    let mut limit = 10usize;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--limit" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--limit requires a value.".to_string()))?;
                limit = value.parse::<usize>().map_err(|_| {
                    CliError::Usage("--limit must be a positive integer.".to_string())
                })?;
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for telemetry-recent command."
                )));
            }
        }
    }

    Ok(TelemetryRecentOptions { limit })
}

pub(crate) fn parse_session_extract_options(
    args: &[String],
) -> Result<SessionExtractOptions, CliError> {
    let mut session_file = None;
    let mut engine = None;
    let mut claims = Vec::new();
    let mut verifier_command = None;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--engine" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--engine requires a value.".to_string()))?;
                engine = Some(parse_search_engine(value)?);
                index += 2;
            }
            "--claim" => {
                claims.push(parse_claim_value(args, index)?);
                index += 2;
            }
            "--verifier-command" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--verifier-command requires a shell command.".to_string())
                })?;
                verifier_command = Some(value.clone());
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for session-extract command."
                )));
            }
        }
    }

    if claims.is_empty() {
        return Err(CliError::Usage(
            "session-extract requires at least one `--claim` statement.".to_string(),
        ));
    }

    Ok(SessionExtractOptions {
        session_file,
        engine,
        claims,
        verifier_command,
    })
}

pub(crate) fn parse_session_synthesize_options(
    args: &[String],
) -> Result<SessionSynthesizeOptions, CliError> {
    let mut session_file = None;
    let mut note_limit = 12;
    let mut format = OutputFormat::Json;
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(value));
                index += 2;
            }
            "--note-limit" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--note-limit requires an integer.".to_string())
                })?;
                note_limit = value.parse().map_err(|_| {
                    CliError::Usage("--note-limit requires an integer.".to_string())
                })?;
                index += 2;
            }
            "--format" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--format requires `json` or `markdown`.".to_string())
                })?;
                format = parse_output_format(value)?;
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for session-synthesize command."
                )));
            }
        }
    }

    Ok(SessionSynthesizeOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("session-synthesize requires `--session-file <path>`.".to_string())
        })?,
        note_limit,
        format,
    })
}

pub(crate) fn parse_output_format(value: &str) -> Result<OutputFormat, CliError> {
    match value {
        "json" => Ok(OutputFormat::Json),
        "markdown" => Ok(OutputFormat::Markdown),
        _ => Err(CliError::Usage(
            "--format requires `json` or `markdown`.".to_string(),
        )),
    }
}

pub(crate) fn parse_memory_steps(args: &[String]) -> Result<usize, CliError> {
    if args.is_empty() {
        return Ok(20);
    }

    if args.len() == 2 && args[0] == "--steps" {
        return args[1].parse().map_err(|_| {
            CliError::Usage("memory-summary --steps requires an integer value.".to_string())
        });
    }

    Err(CliError::Usage(
        "memory-summary accepts only `--steps <even-number>`.".to_string(),
    ))
}

pub(crate) fn parse_source_risk(value: &str) -> Result<SourceRisk, CliError> {
    match value {
        "low" => Ok(SourceRisk::Low),
        "medium" => Ok(SourceRisk::Medium),
        "hostile" => Ok(SourceRisk::Hostile),
        _ => Err(CliError::Usage(format!(
            "Unknown source risk `{value}`. Expected low|medium|hostile."
        ))),
    }
}
