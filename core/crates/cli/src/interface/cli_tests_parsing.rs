use super::*;

#[test]
fn preprocesses_help_and_json_error_flags() {
    let processed = preprocess_cli_args(vec![
        "--json-errors".to_string(),
        "extract".to_string(),
        "--help".to_string(),
    ]);

    assert!(processed.json_errors);
    assert_eq!(
        processed.args,
        vec!["extract".to_string(), "--help".to_string()]
    );
    assert_eq!(
        processed.help_text,
        command_usage("extract"),
        "command help should surface the extract synopsis",
    );
}

#[test]
fn builds_structured_usage_error_payload() {
    let payload = build_cli_error_payload(&CliError::Usage(
        "extract requires `--claim <statement>`.".to_string(),
    ));

    assert_eq!(payload.error, "missing-claim");
    assert_eq!(payload.kind, "usage-error");
    assert_eq!(
        payload.hint.as_deref(),
        Some("provide --claim <statement> at least once."),
    );
}
#[test]
fn parses_extract_command_with_multiple_claims() {
    let command = parse_command(&[
        "extract".to_string(),
        "fixture://research/citation-heavy/pricing".to_string(),
        "--claim".to_string(),
        "The Starter plan costs $29 per month.".to_string(),
        "--claim".to_string(),
        "There is an Enterprise plan.".to_string(),
    ])
    .expect("extract command should parse");

    assert_eq!(
        command,
        CliCommand::Extract(ExtractOptions {
            target: "fixture://research/citation-heavy/pricing".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            session_file: None,
            claims: vec![
                "The Starter plan costs $29 per month.".to_string(),
                "There is an Enterprise plan.".to_string(),
            ],
            verifier_command: None,
        })
    );
}

#[test]
fn rejects_blank_extract_claims() {
    let error = parse_command(&[
        "extract".to_string(),
        "fixture://research/citation-heavy/pricing".to_string(),
        "--claim".to_string(),
        "   ".to_string(),
    ])
    .expect_err("blank extract claim should be rejected");

    assert_eq!(error.to_string(), "--claim requires a non-empty statement.");
}

#[test]
fn rejects_blank_session_extract_claims() {
    let error = parse_command(&[
        "session-extract".to_string(),
        "--claim".to_string(),
        "".to_string(),
    ])
    .expect_err("blank session extract claim should be rejected");

    assert_eq!(error.to_string(), "--claim requires a non-empty statement.");
}

#[test]
fn parses_search_command_with_engine_and_session_file() {
    let command = parse_command(&[
        "search".to_string(),
        "lambda timeout".to_string(),
        "--engine".to_string(),
        "brave".to_string(),
        "--session-file".to_string(),
        "/tmp/search-session.json".to_string(),
        "--headed".to_string(),
    ])
    .expect("search command should parse");

    assert_eq!(
        command,
        CliCommand::Search(SearchOptions {
            query: "lambda timeout".to_string(),
            engine: SearchEngine::Brave,
            budget: DEFAULT_SEARCH_TOKENS,
            headed: true,
            profile_dir: None,
            session_file: Some(PathBuf::from("/tmp/search-session.json")),
        })
    );
}

#[test]
fn parses_search_command_with_profile_dir() {
    let command = parse_command(&[
        "search".to_string(),
        "lambda timeout".to_string(),
        "--profile-dir".to_string(),
        "/tmp/dedicated-search-profile".to_string(),
    ])
    .expect("search command with profile dir should parse");

    assert_eq!(
        command,
        CliCommand::Search(SearchOptions {
            query: "lambda timeout".to_string(),
            engine: SearchEngine::Google,
            budget: DEFAULT_SEARCH_TOKENS,
            headed: false,
            profile_dir: Some(PathBuf::from("/tmp/dedicated-search-profile")),
            session_file: None,
        })
    );
}

#[test]
fn parses_search_open_result_command() {
    let command = parse_command(&[
        "search-open-result".to_string(),
        "--session-file".to_string(),
        "/tmp/search-session.json".to_string(),
        "--prefer-official".to_string(),
        "--rank".to_string(),
        "2".to_string(),
    ])
    .expect("search-open-result command should parse");

    assert_eq!(
        command,
        CliCommand::SearchOpenResult(SearchOpenResultOptions {
            engine: SearchEngine::Google,
            session_file: Some(PathBuf::from("/tmp/search-session.json")),
            rank: 2,
            prefer_official: true,
            headed: false,
        })
    );
}

#[test]
fn parses_session_extract_command_with_engine_hint() {
    let command = parse_command(&[
        "session-extract".to_string(),
        "--engine".to_string(),
        "brave".to_string(),
        "--claim".to_string(),
        "Example claim".to_string(),
    ])
    .expect("session-extract with engine should parse");

    assert_eq!(
        command,
        CliCommand::SessionExtract(SessionExtractOptions {
            session_file: None,
            engine: Some(SearchEngine::Brave),
            claims: vec!["Example claim".to_string()],
            verifier_command: None,
        })
    );
}

#[test]
fn parses_search_open_top_command() {
    let command = parse_command(&[
        "search-open-top".to_string(),
        "--engine".to_string(),
        "brave".to_string(),
        "--limit".to_string(),
        "2".to_string(),
        "--headless".to_string(),
    ])
    .expect("search-open-top command should parse");

    assert_eq!(
        command,
        CliCommand::SearchOpenTop(SearchOpenTopOptions {
            engine: SearchEngine::Brave,
            session_file: None,
            limit: 2,
            headed: false,
        })
    );
}

#[test]
fn parses_extract_command_with_verifier_hook() {
    let command = parse_command(&[
        "extract".to_string(),
        "fixture://research/citation-heavy/pricing".to_string(),
        "--claim".to_string(),
        "The Starter plan costs $29 per month.".to_string(),
        "--verifier-command".to_string(),
        "printf '{\"outcomes\":[]}'".to_string(),
    ])
    .expect("extract command with verifier should parse");

    assert_eq!(
        command,
        CliCommand::Extract(ExtractOptions {
            target: "fixture://research/citation-heavy/pricing".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            session_file: None,
            claims: vec!["The Starter plan costs $29 per month.".to_string()],
            verifier_command: Some("printf '{\"outcomes\":[]}'".to_string()),
        })
    );
}

#[test]
fn parses_session_synthesize_markdown_format() {
    let command = parse_command(&[
        "session-synthesize".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--format".to_string(),
        "markdown".to_string(),
    ])
    .expect("session-synthesize command should parse");

    assert_eq!(
        command,
        CliCommand::SessionSynthesize(SessionSynthesizeOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            note_limit: 12,
            format: OutputFormat::Markdown,
        })
    );
}
#[test]
fn parses_open_command_with_browser_flags() {
    let command = parse_command(&[
        "open".to_string(),
        "fixture://research/static-docs/getting-started".to_string(),
        "--browser".to_string(),
        "--headed".to_string(),
    ])
    .expect("open command should parse");

    assert_eq!(
        command,
        CliCommand::Open(TargetOptions {
            target: "fixture://research/static-docs/getting-started".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: true,
            headed: true,
            main_only: false,
            session_file: None,
        })
    );
}

#[test]
fn parses_open_command_with_custom_budget() {
    let command = parse_command(&[
        "open".to_string(),
        "fixture://research/static-docs/getting-started".to_string(),
        "--budget".to_string(),
        "2048".to_string(),
    ])
    .expect("open command with budget should parse");

    assert_eq!(
        command,
        CliCommand::Open(TargetOptions {
            target: "fixture://research/static-docs/getting-started".to_string(),
            budget: 2048,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            main_only: false,
            session_file: None,
        })
    );
}

#[test]
fn parses_read_view_command_with_main_only() {
    let command = parse_command(&[
        "read-view".to_string(),
        "https://www.iana.org/help/example-domains".to_string(),
        "--main-only".to_string(),
    ])
    .expect("read-view command should parse");

    assert_eq!(
        command,
        CliCommand::ReadView(TargetOptions {
            target: "https://www.iana.org/help/example-domains".to_string(),
            budget: DEFAULT_REQUESTED_TOKENS,
            source_risk: None,
            source_label: None,
            allowlisted_domains: Vec::new(),
            browser: false,
            headed: false,
            main_only: true,
            session_file: None,
        })
    );
}

#[test]
fn parses_session_read_command_with_main_only() {
    let command = parse_command(&[
        "session-read".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--main-only".to_string(),
    ])
    .expect("session-read command should parse");

    assert_eq!(
        command,
        CliCommand::SessionRead(SessionReadOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            main_only: true,
        })
    );
}

#[test]
fn parses_click_command() {
    let command = parse_command(&[
        "click".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--ref".to_string(),
        "rmain:button:continue".to_string(),
        "--headed".to_string(),
    ])
    .expect("click command should parse");

    assert_eq!(
        command,
        CliCommand::Click(ClickOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            target_ref: "rmain:button:continue".to_string(),
            headed: true,
            ack_risks: Vec::new(),
        })
    );
}

#[test]
fn parses_refresh_command() {
    let command = parse_command(&[
        "refresh".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
    ])
    .expect("refresh command should parse");

    assert_eq!(
        command,
        CliCommand::SessionRefresh(SessionRefreshOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            headed: false,
        })
    );
}

#[test]
fn parses_checkpoint_command() {
    let command = parse_command(&[
        "checkpoint".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
    ])
    .expect("checkpoint command should parse");

    assert_eq!(
        command,
        CliCommand::SessionCheckpoint(SessionFileOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
        })
    );
}

#[test]
fn parses_approve_command() {
    let command = parse_command(&[
        "approve".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--risk".to_string(),
        "mfa".to_string(),
        "--risk".to_string(),
        "auth".to_string(),
    ])
    .expect("approve command should parse");

    assert_eq!(
        command,
        CliCommand::Approve(ApproveOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
        })
    );
}

#[test]
fn parses_set_profile_command() {
    let command = parse_command(&[
        "set-profile".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--profile".to_string(),
        "interactive-supervised-auth".to_string(),
    ])
    .expect("set-profile command should parse");

    assert_eq!(
        command,
        CliCommand::SetProfile(SessionProfileSetOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            profile: PolicyProfile::InteractiveSupervisedAuth,
        })
    );
}

#[test]
fn parses_telemetry_recent_command() {
    let command = parse_command(&[
        "telemetry-recent".to_string(),
        "--limit".to_string(),
        "7".to_string(),
    ])
    .expect("telemetry-recent command should parse");

    assert_eq!(
        command,
        CliCommand::TelemetryRecent(TelemetryRecentOptions { limit: 7 })
    );
}

#[test]
fn parses_click_command_with_ack_risk() {
    let command = parse_command(&[
        "click".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--ref".to_string(),
        "rmain:button:continue".to_string(),
        "--ack-risk".to_string(),
        "challenge".to_string(),
        "--ack-risk".to_string(),
        "auth".to_string(),
    ])
    .expect("click command with ack risks should parse");

    assert_eq!(
        command,
        CliCommand::Click(ClickOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            target_ref: "rmain:button:continue".to_string(),
            headed: false,
            ack_risks: vec![AckRisk::Challenge, AckRisk::Auth],
        })
    );
}

#[test]
fn parses_type_command_with_sensitive_flag() {
    let command = parse_command(&[
        "type".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--ref".to_string(),
        "rmain:input:password".to_string(),
        "--value".to_string(),
        "hunter2".to_string(),
        "--sensitive".to_string(),
    ])
    .expect("type command should parse");

    assert_eq!(
        command,
        CliCommand::Type(TypeOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            target_ref: "rmain:input:password".to_string(),
            value: "hunter2".to_string(),
            headed: false,
            sensitive: true,
            ack_risks: Vec::new(),
        })
    );
}

#[test]
fn parses_submit_command() {
    let command = parse_command(&[
        "submit".to_string(),
        "--session-file".to_string(),
        "/tmp/test-session.json".to_string(),
        "--ref".to_string(),
        "rmain:form:sign-in".to_string(),
    ])
    .expect("submit command should parse");

    assert_eq!(
        command,
        CliCommand::Submit(SubmitOptions {
            session_file: PathBuf::from("/tmp/test-session.json"),
            target_ref: "rmain:form:sign-in".to_string(),
            headed: false,
            ack_risks: Vec::new(),
            extra_prefill: Vec::new(),
        })
    );
}
