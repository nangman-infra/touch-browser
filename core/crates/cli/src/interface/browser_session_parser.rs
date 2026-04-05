use std::path::PathBuf;

use crate::{
    CliError, ClickOptions, ExpandOptions, FollowOptions, PaginateOptions, PaginationDirection,
    SubmitOptions, TypeOptions,
};

use super::command_parser::parse_ack_risk;

pub(crate) fn parse_follow_options(args: &[String]) -> Result<FollowOptions, CliError> {
    let mut session_file = None;
    let mut target_ref = None;
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
            "--ref" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ref requires a stable ref.".to_string()))?;
                target_ref = Some(value.clone());
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for follow command."
                )));
            }
        }
    }

    Ok(FollowOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("follow requires `--session-file <path>`.".to_string())
        })?,
        target_ref: target_ref
            .ok_or_else(|| CliError::Usage("follow requires `--ref <stable-ref>`.".to_string()))?,
        headed,
    })
}

pub(crate) fn parse_click_options(args: &[String]) -> Result<ClickOptions, CliError> {
    let mut session_file = None;
    let mut target_ref = None;
    let mut headed = false;
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
            "--ref" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ref requires a stable ref.".to_string()))?;
                target_ref = Some(value.clone());
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            "--ack-risk" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ack-risk requires a value.".to_string()))?;
                ack_risks.push(parse_ack_risk(next)?);
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for click command."
                )));
            }
        }
    }

    Ok(ClickOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("click requires `--session-file <path>`.".to_string())
        })?,
        target_ref: target_ref
            .ok_or_else(|| CliError::Usage("click requires `--ref <stable-ref>`.".to_string()))?,
        headed,
        ack_risks,
    })
}

pub(crate) fn parse_type_options(args: &[String]) -> Result<TypeOptions, CliError> {
    let mut session_file = None;
    let mut target_ref = None;
    let mut value = None;
    let mut headed = false;
    let mut sensitive = false;
    let mut ack_risks = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let next = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(next));
                index += 2;
            }
            "--ref" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ref requires a stable ref.".to_string()))?;
                target_ref = Some(next.clone());
                index += 2;
            }
            "--value" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--value requires text.".to_string()))?;
                value = Some(next.clone());
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            "--sensitive" => {
                sensitive = true;
                index += 1;
            }
            "--ack-risk" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ack-risk requires a value.".to_string()))?;
                ack_risks.push(parse_ack_risk(next)?);
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for type command."
                )));
            }
        }
    }

    Ok(TypeOptions {
        session_file: session_file
            .ok_or_else(|| CliError::Usage("type requires `--session-file <path>`.".to_string()))?,
        target_ref: target_ref
            .ok_or_else(|| CliError::Usage("type requires `--ref <stable-ref>`.".to_string()))?,
        value: value
            .ok_or_else(|| CliError::Usage("type requires `--value <text>`.".to_string()))?,
        headed,
        sensitive,
        ack_risks,
    })
}

pub(crate) fn parse_submit_options(args: &[String]) -> Result<SubmitOptions, CliError> {
    let mut session_file = None;
    let mut target_ref = None;
    let mut headed = false;
    let mut ack_risks = Vec::new();
    let mut index = 0;

    while index < args.len() {
        match args[index].as_str() {
            "--session-file" => {
                let next = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--session-file requires a path.".to_string())
                })?;
                session_file = Some(PathBuf::from(next));
                index += 2;
            }
            "--ref" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ref requires a stable ref.".to_string()))?;
                target_ref = Some(next.clone());
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            "--ack-risk" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ack-risk requires a value.".to_string()))?;
                ack_risks.push(parse_ack_risk(next)?);
                index += 2;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for submit command."
                )));
            }
        }
    }

    Ok(SubmitOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("submit requires `--session-file <path>`.".to_string())
        })?,
        target_ref: target_ref
            .ok_or_else(|| CliError::Usage("submit requires `--ref <stable-ref>`.".to_string()))?,
        headed,
        ack_risks,
        extra_prefill: Vec::new(),
    })
}

pub(crate) fn parse_paginate_options(args: &[String]) -> Result<PaginateOptions, CliError> {
    let mut session_file = None;
    let mut direction = None;
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
            "--direction" => {
                let value = args.get(index + 1).ok_or_else(|| {
                    CliError::Usage("--direction requires `next` or `prev`.".to_string())
                })?;
                direction = Some(match value.as_str() {
                    "next" => PaginationDirection::Next,
                    "prev" => PaginationDirection::Prev,
                    _ => {
                        return Err(CliError::Usage(
                            "--direction requires `next` or `prev`.".to_string(),
                        ))
                    }
                });
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for paginate command."
                )));
            }
        }
    }

    Ok(PaginateOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("paginate requires `--session-file <path>`.".to_string())
        })?,
        direction: direction.ok_or_else(|| {
            CliError::Usage("paginate requires `--direction next|prev`.".to_string())
        })?,
        headed,
    })
}

pub(crate) fn parse_expand_options(args: &[String]) -> Result<ExpandOptions, CliError> {
    let mut session_file = None;
    let mut target_ref = None;
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
            "--ref" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| CliError::Usage("--ref requires a stable ref.".to_string()))?;
                target_ref = Some(value.clone());
                index += 2;
            }
            "--headed" => {
                headed = true;
                index += 1;
            }
            other => {
                return Err(CliError::Usage(format!(
                    "Unknown option `{other}` for expand command."
                )));
            }
        }
    }

    Ok(ExpandOptions {
        session_file: session_file.ok_or_else(|| {
            CliError::Usage("expand requires `--session-file <path>`.".to_string())
        })?,
        target_ref: target_ref
            .ok_or_else(|| CliError::Usage("expand requires `--ref <stable-ref>`.".to_string()))?,
        headed,
    })
}
