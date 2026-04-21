use super::deps::{
    usage, AckRisk, CliCommand, CliError, OutputFormat, PolicyProfile, SearchEngine, SourceRisk,
};
use super::{
    browser_session_parser, install_command_parser, search_command_parser, session_command_parser,
};

pub(crate) fn parse_command(args: &[String]) -> Result<CliCommand, CliError> {
    let Some(command_name) = args.first().map(String::as_str) else {
        return Err(CliError::Usage(usage()));
    };

    match command_name {
        "capabilities" | "status" => Ok(CliCommand::Capabilities),
        "search" => Ok(CliCommand::Search(
            search_command_parser::parse_search_options(&args[1..])?,
        )),
        "search-open-result" => Ok(CliCommand::SearchOpenResult(
            search_command_parser::parse_search_open_result_options(&args[1..])?,
        )),
        "search-open-top" => Ok(CliCommand::SearchOpenTop(
            search_command_parser::parse_search_open_top_options(&args[1..])?,
        )),
        "mcp" => Ok(CliCommand::Mcp),
        "update" => Ok(CliCommand::Update(
            install_command_parser::parse_update_options(&args[1..])?,
        )),
        "uninstall" => Ok(CliCommand::Uninstall(
            install_command_parser::parse_uninstall_options(&args[1..])?,
        )),
        "open" => Ok(CliCommand::Open(
            session_command_parser::parse_target_options(&args[1..])?,
        )),
        "snapshot" => Ok(CliCommand::Snapshot(
            session_command_parser::parse_target_options(&args[1..])?,
        )),
        "compact-view" => Ok(CliCommand::CompactView(
            session_command_parser::parse_target_options(&args[1..])?,
        )),
        "read-view" => Ok(CliCommand::ReadView(
            session_command_parser::parse_target_options(&args[1..])?,
        )),
        "extract" => Ok(CliCommand::Extract(
            session_command_parser::parse_extract_options(&args[1..])?,
        )),
        "policy" => Ok(CliCommand::Policy(
            session_command_parser::parse_target_options(&args[1..])?,
        )),
        "session-snapshot" => Ok(CliCommand::SessionSnapshot(
            session_command_parser::parse_session_file_options(&args[1..], "session-snapshot")?,
        )),
        "session-compact" => Ok(CliCommand::SessionCompact(
            session_command_parser::parse_session_file_options(&args[1..], "session-compact")?,
        )),
        "refresh" => Ok(CliCommand::SessionRefresh(
            session_command_parser::parse_session_refresh_options(&args[1..])?,
        )),
        "session-extract" => Ok(CliCommand::SessionExtract(
            session_command_parser::parse_session_extract_options(&args[1..])?,
        )),
        "session-read" => Ok(CliCommand::SessionRead(
            session_command_parser::parse_session_read_options(&args[1..])?,
        )),
        "checkpoint" => Ok(CliCommand::SessionCheckpoint(
            session_command_parser::parse_session_file_options(&args[1..], "checkpoint")?,
        )),
        "session-policy" => Ok(CliCommand::SessionPolicy(
            session_command_parser::parse_session_file_options(&args[1..], "session-policy")?,
        )),
        "session-profile" => Ok(CliCommand::SessionProfile(
            session_command_parser::parse_session_file_options(&args[1..], "session-profile")?,
        )),
        "set-profile" => Ok(CliCommand::SetProfile(
            session_command_parser::parse_set_profile_options(&args[1..])?,
        )),
        "session-synthesize" => Ok(CliCommand::SessionSynthesize(
            session_command_parser::parse_session_synthesize_options(&args[1..])?,
        )),
        "approve" => Ok(CliCommand::Approve(
            session_command_parser::parse_approve_options(&args[1..])?,
        )),
        "follow" => Ok(CliCommand::Follow(
            browser_session_parser::parse_follow_options(&args[1..])?,
        )),
        "click" => Ok(CliCommand::Click(
            browser_session_parser::parse_click_options(&args[1..])?,
        )),
        "type" => Ok(CliCommand::Type(
            browser_session_parser::parse_type_options(&args[1..])?,
        )),
        "submit" => Ok(CliCommand::Submit(
            browser_session_parser::parse_submit_options(&args[1..])?,
        )),
        "paginate" => Ok(CliCommand::Paginate(
            browser_session_parser::parse_paginate_options(&args[1..])?,
        )),
        "expand" => Ok(CliCommand::Expand(
            browser_session_parser::parse_expand_options(&args[1..])?,
        )),
        "browser-replay" => Ok(CliCommand::BrowserReplay(
            session_command_parser::parse_session_file_options(&args[1..], "browser-replay")?,
        )),
        "session-close" => Ok(CliCommand::SessionClose(
            session_command_parser::parse_session_file_options(&args[1..], "session-close")?,
        )),
        "telemetry-summary" => Ok(CliCommand::TelemetrySummary),
        "telemetry-recent" => Ok(CliCommand::TelemetryRecent(
            session_command_parser::parse_telemetry_recent_options(&args[1..])?,
        )),
        "replay" => {
            let scenario = args
                .get(1)
                .cloned()
                .ok_or_else(|| CliError::Usage("replay requires a scenario name.".to_string()))?;
            Ok(CliCommand::Replay { scenario })
        }
        "memory-summary" => Ok(CliCommand::MemorySummary {
            steps: session_command_parser::parse_memory_steps(&args[1..])?,
        }),
        "serve" => Ok(CliCommand::Serve),
        _ => Err(CliError::Usage(format!(
            "Unknown command `{command_name}`.\n\n{}",
            usage()
        ))),
    }
}

pub(crate) fn parse_search_engine(value: &str) -> Result<SearchEngine, CliError> {
    search_command_parser::parse_search_engine(value)
}

pub(crate) fn parse_ack_risk(value: &str) -> Result<AckRisk, CliError> {
    session_command_parser::parse_ack_risk(value)
}

pub(crate) fn parse_policy_profile(value: &str) -> Result<PolicyProfile, CliError> {
    session_command_parser::parse_policy_profile(value)
}

pub(crate) fn parse_output_format(value: &str) -> Result<OutputFormat, CliError> {
    session_command_parser::parse_output_format(value)
}

pub(crate) fn parse_source_risk(value: &str) -> Result<SourceRisk, CliError> {
    session_command_parser::parse_source_risk(value)
}
