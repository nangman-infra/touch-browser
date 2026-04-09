use super::*;

#[test]
fn replays_browser_trace_into_new_browser_session() {
    let session_file = temp_session_path("browser-replay");
    let open_output = open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-follow",
    );
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "link")
        .and_then(|block| block["ref"].as_str())
        .expect("link ref should exist")
        .to_string();

    dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref,
        headed: false,
    }))
    .expect("follow should succeed");

    let replay_output = dispatch(CliCommand::BrowserReplay(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("browser replay should succeed");

    assert_eq!(replay_output["replayedActions"], 1);
    assert!(replay_output["compactText"]
        .as_str()
        .expect("compact text should exist")
        .contains("Advanced guide opened"));
    assert_eq!(
        replay_output["sessionState"]["currentUrl"],
        "fixture://research/navigation/browser-follow"
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn session_close_removes_browser_context_directory() {
    let session_file = temp_session_path("browser-context-cleanup");
    open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-follow",
    );

    let context_dir = browser_context_dir_for_session_file(&session_file);
    assert!(context_dir.exists(), "browser context dir should exist");

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");

    assert!(
        !context_dir.exists(),
        "browser context dir should be removed on close"
    );
}

#[test]
fn session_close_preserves_external_profile_directory() {
    let session_file = temp_session_path("browser-profile-preserve");
    let profile_dir = std::env::temp_dir().join(format!(
        "touch-browser-preserved-profile-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos()
    ));
    fs::create_dir_all(&profile_dir).expect("external profile dir should exist");

    open_browser_session(
        &session_file,
        "fixture://research/navigation/browser-follow",
    );

    let mut persisted =
        load_browser_cli_session(&session_file).expect("session should load after open");
    if let Some(context_dir) = persisted.browser_context_dir.as_ref() {
        let context_path = PathBuf::from(context_dir);
        if context_path.exists() {
            fs::remove_dir_all(context_path).expect("managed context dir should clean up");
        }
    }
    persisted.browser_context_dir = None;
    persisted.browser_profile_dir = Some(profile_dir.display().to_string());
    save_browser_cli_session(&session_file, &persisted)
        .expect("session should save external profile state");

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");

    assert!(
        profile_dir.exists(),
        "external profile dir should not be removed on close"
    );

    fs::remove_dir_all(&profile_dir).expect("external profile dir cleanup should succeed");
}

#[test]
fn browser_open_appends_existing_session_history() {
    let server = CliTestServer::start();
    let session_file = temp_session_path("browser-open-append-history");

    open_browser_session(&session_file, server.url("/static"));

    open_browser_session(&session_file, server.url("/docs-shell"));

    let persisted =
        load_browser_cli_session(&session_file).expect("session should load after two opens");
    assert_eq!(persisted.session.snapshots.len(), 2);
    assert_eq!(persisted.session.state.visited_urls.len(), 2);
    assert_eq!(
        persisted.session.state.visited_urls,
        vec![server.url("/static"), server.url("/docs-shell")]
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn browser_session_outputs_strip_markup_and_extract_selector_availability() {
    let server = CliTestServer::start();
    let session_file = temp_session_path("polluted-selector-session");

    open_browser_session(&session_file, server.url("/polluted-selector"));

    let compact = dispatch(CliCommand::SessionCompact(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session compact should succeed");
    let compact_text = compact["compactText"]
        .as_str()
        .expect("compact text should exist");
    let reading_compact = compact["readingCompactText"]
        .as_str()
        .expect("reading compact text should exist");
    assert!(compact_text.contains("Get Node.js v24.14.1"));
    assert!(compact_text.contains("macOS"));
    assert!(!compact_text.contains("<style>"));
    assert!(!compact_text.contains("index-module__select"));
    assert!(!reading_compact.contains("<style>"));
    assert!(!reading_compact.contains("index-module__select"));

    let read = dispatch(CliCommand::SessionRead(SessionReadOptions {
        session_file: session_file.clone(),
        main_only: true,
    }))
    .expect("session read should succeed");
    let markdown = read["markdownText"]
        .as_str()
        .expect("markdown text should exist");
    assert!(markdown.contains("Get Node.js v24.14.1"));
    assert!(markdown.contains("macOS"));
    assert!(!markdown.contains("<style>"));
    assert!(!markdown.contains("index-module__select"));

    let synthesis = dispatch(CliCommand::SessionSynthesize(SessionSynthesizeOptions {
        session_file: session_file.clone(),
        note_limit: 8,
        format: OutputFormat::Markdown,
    }))
    .expect("session synthesis should succeed");
    let synthesis_markdown = synthesis["markdown"]
        .as_str()
        .expect("session synthesis markdown should exist");
    assert!(synthesis_markdown.contains("Get Node.js v24.14.1"));
    assert!(!synthesis_markdown.contains("<style>"));
    assert!(!synthesis_markdown.contains("index-module__select"));

    let extract = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: Some(session_file.clone()),
        engine: None,
        claims: vec!["Node.js is available for macOS.".to_string()],
        verifier_command: None,
    }))
    .expect("session extract should succeed");
    assert_eq!(extract["extract"]["status"], "succeeded");
    assert_eq!(
        extract["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Node.js is available for macOS."
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}
