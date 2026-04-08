use super::*;

#[test]
fn dispatches_browser_backed_fixture_open() {
    let output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/static-docs/getting-started".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("browser-backed open should succeed");

    assert_eq!(output["status"], "succeeded");
    assert_eq!(output["output"]["source"]["sourceType"], "playwright");
    assert_eq!(output["policy"]["decision"], "allow");
}

#[test]
fn open_stays_on_http_for_static_pages_by_default() {
    let server = CliTestServer::start();
    let output = dispatch(CliCommand::Open(TargetOptions {
        target: server.url("/static"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("static page should open over http");

    assert_eq!(output["output"]["source"]["sourceType"], "http");
    assert_eq!(output["output"]["source"]["title"], "Static Docs");
}

#[test]
fn read_view_auto_falls_back_to_browser_for_js_shell_pages() {
    let server = CliTestServer::start();
    let output = dispatch(CliCommand::ReadView(TargetOptions {
        target: server.url("/spa"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("js shell read-view should fallback to browser");

    let markdown = output["markdownText"]
        .as_str()
        .expect("markdown text should be present");
    assert!(markdown.contains("Client Rendered Docs"));
    assert!(markdown.contains("The browser runtime can read JS apps."));
}

#[test]
fn open_auto_falls_back_to_browser_for_shell_heavy_docs_pages() {
    let server = CliTestServer::start();
    let output = dispatch(CliCommand::Open(TargetOptions {
        target: server.url("/docs-shell"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("shell-heavy docs page should fallback to browser");

    assert_eq!(output["output"]["source"]["sourceType"], "playwright");
    assert_eq!(output["output"]["source"]["title"], "Docs Shell");
}

#[test]
fn read_view_auto_falls_back_to_browser_for_shell_heavy_docs_pages() {
    let server = CliTestServer::start();
    let output = dispatch(CliCommand::ReadView(TargetOptions {
        target: server.url("/docs-shell"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: true,
        session_file: None,
    }))
    .expect("shell-heavy docs read-view should fallback to browser");

    let markdown = output["markdownText"]
        .as_str()
        .expect("markdown text should be present");
    assert!(markdown.contains("Rendered Guide"));
    assert!(markdown.contains("recover shell-heavy docs pages"));
    assert!(!markdown.contains("Getting Started"));
}

#[test]
fn extract_auto_falls_back_to_browser_for_js_shell_pages() {
    let server = CliTestServer::start();
    let output = dispatch(CliCommand::Extract(ExtractOptions {
        target: server.url("/spa"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        session_file: None,
        claims: vec!["The browser runtime can read JS apps.".to_string()],
        verifier_command: None,
    }))
    .expect("js shell extract should fallback to browser");

    assert_eq!(
        output["open"]["output"]["source"]["sourceType"],
        "playwright"
    );
    assert_eq!(
        output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "The browser runtime can read JS apps."
    );
}

#[test]
fn dispatches_browser_backed_extract() {
    let output = dispatch(CliCommand::Extract(ExtractOptions {
        target: "fixture://research/citation-heavy/pricing".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        session_file: None,
        claims: vec!["The Starter plan costs $29 per month.".to_string()],
        verifier_command: None,
    }))
    .expect("browser-backed extract should succeed");

    assert_eq!(
        output["open"]["output"]["source"]["sourceType"],
        "playwright"
    );
    assert_eq!(output["extract"]["status"], "succeeded");
    assert_eq!(
        output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "The Starter plan costs $29 per month."
    );
}

#[test]
fn attaches_verifier_outcomes_to_extract_results() {
    let output = dispatch(CliCommand::Extract(ExtractOptions {
        target: "fixture://research/citation-heavy/pricing".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        session_file: None,
        claims: vec!["The Starter plan costs $29 per month.".to_string()],
        verifier_command: Some(
            "printf '{\"outcomes\":[{\"claimId\":\"c1\",\"verdict\":\"verified\",\"verifierScore\":0.88,\"notes\":\"checked against source\"}]}'"
                .to_string(),
        ),
    }))
    .expect("extract with verifier should succeed");

    assert_eq!(
        output["extract"]["output"]["verification"]["outcomes"][0]["verdict"],
        "verified"
    );
    assert_eq!(
        output["extract"]["output"]["verification"]["outcomes"][0]["verifierScore"],
        0.88
    );
    assert_eq!(
        output["extract"]["output"]["claimOutcomes"][0]["verdict"],
        "evidence-supported"
    );
}

#[test]
fn verifier_can_demote_supported_claims_into_needs_more_browsing() {
    let output = dispatch(CliCommand::Extract(ExtractOptions {
        target: "fixture://research/citation-heavy/pricing".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        session_file: None,
        claims: vec!["The Starter plan costs $29 per month.".to_string()],
        verifier_command: Some(
            "printf '{\"outcomes\":[{\"claimId\":\"c1\",\"verdict\":\"needs-more-browsing\",\"verifierScore\":0.31,\"notes\":\"open a more specific pricing table before answering\"}]}'"
                .to_string(),
        ),
    }))
    .expect("extract with demoting verifier should succeed");

    assert_eq!(
        output["extract"]["output"]["evidenceSupportedClaims"]
            .as_array()
            .expect("supported claims should be present")
            .len(),
        0
    );
    assert_eq!(
        output["extract"]["output"]["needsMoreBrowsingClaims"][0]["statement"],
        "The Starter plan costs $29 per month."
    );
    assert_eq!(
        output["extract"]["output"]["claimOutcomes"][0]["verificationVerdict"],
        "needs-more-browsing"
    );
}

#[test]
fn dispatches_browser_backed_hostile_policy() {
    let output = dispatch(CliCommand::Policy(TargetOptions {
        target: "fixture://research/hostile/fake-system-message".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("browser-backed policy should succeed");

    assert_eq!(output["policy"]["decision"], "block");
    assert_eq!(output["policy"]["riskClass"], "blocked");
}

#[test]
fn persists_browser_session_and_reads_current_snapshot() {
    let session_file = temp_session_path("session-open");
    let output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-pagination".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

    assert_eq!(output["status"], "succeeded");
    assert!(session_file.exists());

    let snapshot = dispatch(CliCommand::SessionSnapshot(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session snapshot should succeed");

    assert_eq!(snapshot["action"]["status"], "succeeded");
    assert_eq!(
        snapshot["action"]["output"]["blocks"][1]["text"],
        "Browser Pagination"
    );

    fs::remove_file(session_file).ok();
}

#[test]
fn refreshes_browser_session_from_current_live_state() {
    let session_file = temp_session_path("session-refresh");
    dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

    let refreshed = dispatch(CliCommand::SessionRefresh(SessionRefreshOptions {
        session_file: session_file.clone(),
        headed: false,
    }))
    .expect("refresh should succeed");

    assert_eq!(refreshed["action"]["status"], "succeeded");
    assert_eq!(refreshed["action"]["action"], "read");

    fs::remove_file(session_file).ok();
}

#[test]
fn paginates_browser_session_and_updates_snapshot() {
    let session_file = temp_session_path("session-paginate");
    dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-pagination".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

    let output = dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: session_file.clone(),
        direction: PaginationDirection::Next,
        headed: false,
    }))
    .expect("paginate should succeed");

    assert_eq!(output["action"]["status"], "succeeded");
    assert_eq!(output["action"]["action"], "paginate");
    assert_eq!(output["action"]["output"]["adapter"]["page"], 2);
    assert!(output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Page 2"));

    fs::remove_file(session_file).ok();
}

#[test]
fn preserves_browser_dom_state_across_paginate_actions() {
    let session_file = temp_session_path("session-paginate-twice");
    dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-pagination".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

    dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: session_file.clone(),
        direction: PaginationDirection::Next,
        headed: false,
    }))
    .expect("first paginate should succeed");

    let second_paginate = dispatch(CliCommand::Paginate(PaginateOptions {
        session_file: session_file.clone(),
        direction: PaginationDirection::Next,
        headed: false,
    }))
    .expect_err("second paginate should fail after the next button disappears");

    assert!(
        second_paginate
            .to_string()
            .contains("No next pagination target was found."),
        "unexpected error: {second_paginate}"
    );

    fs::remove_file(session_file).ok();
}

#[test]
fn follows_browser_session_and_can_extract_from_persisted_state() {
    let session_file = temp_session_path("session-follow");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "link")
        .and_then(|block| block["ref"].as_str())
        .expect("link ref should exist")
        .to_string();

    let follow_output = dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref,
        headed: false,
    }))
    .expect("follow should succeed");

    assert_eq!(follow_output["action"]["status"], "succeeded");
    assert_eq!(follow_output["action"]["action"], "follow");
    assert_eq!(follow_output["result"]["status"], "succeeded");
    assert!(follow_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Advanced guide opened for the next research step."));

    let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: Some(session_file.clone()),
        engine: None,
        claims: vec!["Advanced guide opened for the next research step.".to_string()],
        verifier_command: None,
    }))
    .expect("session extract should succeed");

    assert_eq!(extract_output["extract"]["status"], "succeeded");
    assert_eq!(extract_output["result"]["status"], "succeeded");
    assert_eq!(
        extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Advanced guide opened for the next research step."
    );

    let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");
    assert_eq!(close_output["removed"], true);
}

#[test]
fn preserves_requested_budget_across_browser_follow_actions() {
    let session_file = temp_session_path("session-follow-budget");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: 64,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "link")
        .and_then(|block| block["ref"].as_str())
        .expect("link ref should exist")
        .to_string();

    let follow_output = dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref,
        headed: false,
    }))
    .expect("follow should succeed");

    assert_eq!(
        follow_output["action"]["output"]["snapshot"]["budget"]["requestedTokens"],
        64
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn follows_duplicate_browser_link_using_stable_ref_ordinal_hint() {
    let session_file = temp_session_path("session-follow-duplicate");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow-duplicate".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let follow_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .filter(|block| block["kind"] == "link")
        .find(|block| {
            block["ref"]
                .as_str()
                .expect("ref should be present")
                .ends_with(":2")
        })
        .and_then(|block| block["ref"].as_str())
        .expect("second link ref should exist")
        .to_string();

    let follow_output = dispatch(CliCommand::Follow(FollowOptions {
        session_file: session_file.clone(),
        target_ref: follow_ref,
        headed: false,
    }))
    .expect("follow should succeed");

    assert_eq!(follow_output["action"]["status"], "succeeded");
    assert!(follow_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Current docs opened for the research step."));

    let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");
    assert_eq!(close_output["removed"], true);
}

#[test]
fn expands_browser_session_and_can_extract_from_persisted_state() {
    let session_file = temp_session_path("session-expand");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-expand".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let expand_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "button")
        .and_then(|block| block["ref"].as_str())
        .expect("button ref should exist")
        .to_string();

    let expand_output = dispatch(CliCommand::Expand(ExpandOptions {
        session_file: session_file.clone(),
        target_ref: expand_ref,
        headed: false,
    }))
    .expect("expand should succeed");

    assert_eq!(expand_output["action"]["status"], "succeeded");
    assert!(expand_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Expanded details confirm"));

    let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: Some(session_file.clone()),
        engine: None,
        claims: vec![
            "Expanded details confirm that the runtime can reveal collapsed notes.".to_string(),
        ],
        verifier_command: None,
    }))
    .expect("session extract should succeed");

    assert_eq!(extract_output["extract"]["status"], "succeeded");
    assert_eq!(
        extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Expanded details confirm that the runtime can reveal collapsed notes."
    );

    let close_output = dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should succeed");
    assert_eq!(close_output["removed"], true);
}

#[test]
fn session_extract_uses_latest_search_session_when_path_is_omitted() {
    let search_output_dir = repo_root().join("output/browser-search");
    fs::create_dir_all(&search_output_dir).expect("search output dir should exist");
    let session_file = search_output_dir.join(format!(
        "session-extract-default-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos()
    ));

    let runtime = touch_browser_runtime::ReadOnlyRuntime::default();
    let mut session = runtime.start_session("stest-default-extract", DEFAULT_OPENED_AT);
    let snapshot = SnapshotDocument {
        version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
        stable_ref_version: touch_browser_contracts::STABLE_REF_VERSION.to_string(),
        source: SnapshotSource {
            source_url: "https://example.com/default-extract".to_string(),
            source_type: SourceType::Fixture,
            title: Some("Default Extract".to_string()),
        },
        budget: SnapshotBudget {
            requested_tokens: 128,
            estimated_tokens: 24,
            emitted_tokens: 24,
            truncated: false,
        },
        blocks: vec![SnapshotBlock {
            version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
            id: "b1".to_string(),
            kind: SnapshotBlockKind::Text,
            stable_ref: "rmain:text:default".to_string(),
            role: SnapshotBlockRole::Content,
            text: "Latest session extraction target.".to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://example.com/default-extract".to_string(),
                source_type: SourceType::Fixture,
                dom_path_hint: Some("html > body > main > p".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        }],
    };
    runtime
        .open_snapshot(
            &mut session,
            "https://example.com/default-extract",
            snapshot,
            touch_browser_contracts::SourceRisk::Low,
            None,
            DEFAULT_OPENED_AT,
        )
        .expect("snapshot should open");
    save_browser_cli_session(
        &session_file,
        &build_browser_cli_session(
            &session,
            128,
            true,
            Some(PersistedBrowserState {
                current_url: "https://example.com/default-extract".to_string(),
                current_html: "<html><body><main><p>Latest session extraction target.</p></main></body></html>".to_string(),
            }),
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            None,
        ),
    )
    .expect("session file should save");

    let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: None,
        engine: None,
        claims: vec!["Latest session extraction target.".to_string()],
        verifier_command: None,
    }))
    .expect("session extract should use latest search session");

    assert_eq!(
        extract_output["sessionFile"],
        session_file.display().to_string()
    );
    assert_eq!(
        extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Latest session extraction target."
    );

    fs::remove_file(&session_file).expect("session file should be removable");
}

#[test]
fn session_extract_can_resolve_engine_default_search_session() {
    let google_session_file = default_search_session_file(SearchEngine::Google);
    let brave_session_file = default_search_session_file(SearchEngine::Brave);
    if let Some(parent) = google_session_file.parent() {
        fs::create_dir_all(parent).expect("search output dir should exist");
    }

    let runtime = touch_browser_runtime::ReadOnlyRuntime::default();
    let build_session = |session_id: &str, text: &str| {
        let mut session = runtime.start_session(session_id, DEFAULT_OPENED_AT);
        runtime
            .open_snapshot(
                &mut session,
                "https://example.com/engine-extract",
                SnapshotDocument {
                    version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
                    stable_ref_version: touch_browser_contracts::STABLE_REF_VERSION.to_string(),
                    source: SnapshotSource {
                        source_url: "https://example.com/engine-extract".to_string(),
                        source_type: SourceType::Fixture,
                        title: Some("Engine Extract".to_string()),
                    },
                    budget: SnapshotBudget {
                        requested_tokens: 128,
                        estimated_tokens: 24,
                        emitted_tokens: 24,
                        truncated: false,
                    },
                    blocks: vec![SnapshotBlock {
                        version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
                        id: "b1".to_string(),
                        kind: SnapshotBlockKind::Text,
                        stable_ref: "rmain:text:engine".to_string(),
                        role: SnapshotBlockRole::Content,
                        text: text.to_string(),
                        attributes: Default::default(),
                        evidence: SnapshotEvidence {
                            source_url: "https://example.com/engine-extract".to_string(),
                            source_type: SourceType::Fixture,
                            dom_path_hint: Some("html > body > main > p".to_string()),
                            byte_range_start: None,
                            byte_range_end: None,
                        },
                    }],
                },
                touch_browser_contracts::SourceRisk::Low,
                None,
                DEFAULT_OPENED_AT,
            )
            .expect("snapshot should open");
        build_browser_cli_session(
            &session,
            128,
            true,
            Some(PersistedBrowserState {
                current_url: "https://example.com/engine-extract".to_string(),
                current_html: format!("<html><body><main><p>{text}</p></main></body></html>"),
            }),
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            None,
        )
    };

    save_browser_cli_session(
        &google_session_file,
        &build_session("stest-google-engine", "Google engine target."),
    )
    .expect("google search session should save");
    save_browser_cli_session(
        &brave_session_file,
        &build_session("stest-brave-engine", "Brave engine target."),
    )
    .expect("brave search session should save");

    let extract_output = dispatch(CliCommand::SessionExtract(SessionExtractOptions {
        session_file: None,
        engine: Some(SearchEngine::Brave),
        claims: vec!["Brave engine target.".to_string()],
        verifier_command: None,
    }))
    .expect("session extract should use engine-specific session");

    assert_eq!(
        extract_output["sessionFile"],
        brave_session_file.display().to_string()
    );
    assert_eq!(
        extract_output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "Brave engine target."
    );

    let _ = fs::remove_file(google_session_file);
    let _ = fs::remove_file(brave_session_file);
}

#[test]
fn missing_session_file_error_includes_path() {
    let missing = temp_session_path("missing-session-error");
    let error = dispatch(CliCommand::SessionSnapshot(SessionFileOptions {
        session_file: missing.clone(),
    }))
    .expect_err("missing session file should fail");

    let message = error.to_string();
    assert!(message.contains(&missing.display().to_string()));
    assert!(message.contains("No such file or directory"));
}

#[test]
fn types_into_browser_session_and_marks_session_interactive() {
    let session_file = temp_session_path("session-type");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let email_ref = browser_session_block_ref(&open_output, "input", Some("agent@example.com"));

    let type_output = dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: email_ref,
        value: "agent@example.com".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type should succeed");

    assert_eq!(type_output["action"]["status"], "succeeded");
    assert_eq!(type_output["action"]["action"], "type");
    assert_eq!(type_output["sessionState"]["mode"], "interactive");
    assert_eq!(
        type_output["sessionState"]["policyProfile"],
        "interactive-review"
    );
    assert_eq!(
        type_output["action"]["output"]["adapter"]["typedLength"],
        17
    );
    assert!(type_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("agent@example.com"));

    close_browser_fixture_session(session_file);
}

#[test]
fn rejects_sensitive_type_without_explicit_opt_in() {
    let session_file = temp_session_path("session-type-sensitive");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let password_ref = browser_session_block_ref(&open_output, "input", Some("password"));

    let type_output = dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: password_ref,
        value: "hunter2".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type command should return a rejection");

    assert_eq!(type_output["action"]["status"], "rejected");
    assert_eq!(type_output["action"]["failureKind"], "policy-blocked");

    close_browser_fixture_session(session_file);
}

#[test]
fn clicks_browser_session_button_after_interactive_typing() {
    let session_file = temp_session_path("session-click");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let email_ref = browser_session_block_ref(&open_output, "input", Some("agent@example.com"));
    let button_ref = browser_session_block_ref(&open_output, "button", None);

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: email_ref,
        value: "agent@example.com".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type should succeed");

    let click_output = dispatch(CliCommand::Click(ClickOptions {
        session_file: session_file.clone(),
        target_ref: button_ref,
        headed: false,
        ack_risks: vec![AckRisk::Auth],
    }))
    .expect("click should succeed");

    assert_eq!(click_output["action"]["status"], "succeeded");
    assert_eq!(click_output["action"]["action"], "click");
    assert!(click_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Signed in draft session ready for review."));

    close_browser_fixture_session(session_file);
}

#[test]
fn submits_browser_session_form_after_interactive_typing() {
    let session_file = temp_session_path("session-submit");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-login-form",
    );
    let email_ref = browser_session_block_ref(&open_output, "input", Some("agent@example.com"));
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: email_ref,
        value: "agent@example.com".to_string(),
        headed: false,
        sensitive: false,
        ack_risks: Vec::new(),
    }))
    .expect("type should succeed");

    let submit_output = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("submit should succeed");

    assert_eq!(submit_output["action"]["status"], "succeeded");
    assert_eq!(submit_output["action"]["action"], "submit");
    assert!(submit_output["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Signed in draft session ready for review."));

    close_browser_fixture_session(session_file);
}

#[test]
fn rejects_mfa_submit_without_ack_and_allows_it_with_ack() {
    let session_file = temp_session_path("session-mfa");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-mfa-challenge",
    );
    let otp_ref = browser_session_block_ref(&open_output, "input", None);
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    let blocked = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref.clone(),
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("submit should return a rejection");
    assert_eq!(blocked["action"]["status"], "rejected");

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: otp_ref,
        value: "123456".to_string(),
        headed: false,
        sensitive: true,
        ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
    }))
    .expect("sensitive MFA type should succeed");

    let approved = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
        extra_prefill: Vec::new(),
    }))
    .expect("approved submit should succeed");
    assert_eq!(approved["action"]["status"], "succeeded");
    assert!(approved["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Verification code accepted for supervised review."));

    close_browser_fixture_session(session_file);
}

#[test]
fn checkpoint_and_approve_enable_supervised_session_without_repeating_ack_flags() {
    let session_file = temp_session_path("session-checkpoint-approve");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-mfa-challenge",
    );
    let otp_ref = browser_session_block_ref(&open_output, "input", None);
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    let checkpoint = dispatch(CliCommand::SessionCheckpoint(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("checkpoint should succeed");
    assert!(checkpoint["checkpoint"]["requiredAckRisks"]
        .as_array()
        .expect("required risks should be an array")
        .iter()
        .any(|risk| risk == "mfa"));
    assert!(checkpoint["checkpoint"]["requiredAckRisks"]
        .as_array()
        .expect("required risks should be an array")
        .iter()
        .any(|risk| risk == "auth"));
    assert_eq!(
        checkpoint["checkpoint"]["recommendedPolicyProfile"],
        "interactive-supervised-auth"
    );
    assert_eq!(
        checkpoint["checkpoint"]["playbook"]["provider"],
        "generic-auth"
    );

    let approval = dispatch(CliCommand::Approve(ApproveOptions {
        session_file: session_file.clone(),
        ack_risks: vec![AckRisk::Mfa, AckRisk::Auth],
    }))
    .expect("approve should succeed");
    assert!(approval["approvedRisks"]
        .as_array()
        .expect("approved risks should be an array")
        .iter()
        .any(|risk| risk == "mfa"));
    assert_eq!(approval["policyProfile"], "interactive-supervised-auth");

    dispatch(CliCommand::Type(TypeOptions {
        session_file: session_file.clone(),
        target_ref: otp_ref,
        value: "123456".to_string(),
        headed: false,
        sensitive: true,
        ack_risks: Vec::new(),
    }))
    .expect("approved MFA type should succeed without inline ack");

    let approved = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("approved submit should succeed without inline ack");
    assert_eq!(approved["action"]["status"], "succeeded");

    close_browser_fixture_session(session_file);
}

#[test]
fn rejects_high_risk_submit_without_ack_and_allows_it_with_ack() {
    let session_file = temp_session_path("session-high-risk");
    let open_output = open_browser_fixture_session(
        &session_file,
        "fixture://research/navigation/browser-high-risk-checkout",
    );
    let form_ref = browser_session_block_ref(&open_output, "form", None);

    let blocked = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref.clone(),
        headed: false,
        ack_risks: Vec::new(),
        extra_prefill: Vec::new(),
    }))
    .expect("submit should return a rejection");
    assert_eq!(blocked["action"]["status"], "rejected");

    let approved = dispatch(CliCommand::Submit(SubmitOptions {
        session_file: session_file.clone(),
        target_ref: form_ref,
        headed: false,
        ack_risks: vec![AckRisk::HighRiskWrite],
        extra_prefill: Vec::new(),
    }))
    .expect("approved submit should succeed");
    assert_eq!(approved["action"]["status"], "succeeded");
    assert!(approved["action"]["output"]["adapter"]["visibleText"]
        .as_str()
        .expect("visible text should be present")
        .contains("Purchase confirmation staged for supervised review."));

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn dispatches_compact_view_for_fixture() {
    let output = dispatch(CliCommand::CompactView(TargetOptions {
        target: "fixture://research/static-docs/getting-started".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("compact view should succeed");

    assert_eq!(
        output["sourceUrl"],
        "fixture://research/static-docs/getting-started"
    );
    assert!(output["compactText"]
        .as_str()
        .expect("compact text should exist")
        .contains("Getting Started"));
    assert!(output["readingCompactText"]
        .as_str()
        .expect("reading compact text should exist")
        .contains("Getting Started"));
    assert!(output["navigationCompactText"]
        .as_str()
        .expect("navigation compact text should exist")
        .contains("Docs"));
    assert_ne!(
        output["compactText"], output["navigationCompactText"],
        "compact and navigation outputs should remain distinct surfaces",
    );
    assert!(
        output["lineCount"]
            .as_u64()
            .expect("line count should be numeric")
            > 0
    );
}

#[test]
fn dispatches_session_compact_for_browser_session() {
    let session_file = temp_session_path("session-compact");
    dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

    let output = dispatch(CliCommand::SessionCompact(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session compact should succeed");

    assert_eq!(output["sessionFile"], session_file.display().to_string());
    assert!(output["compactText"]
        .as_str()
        .expect("compact text should exist")
        .contains("Browser Follow"));
    assert!(output["readingCompactText"]
        .as_str()
        .expect("reading compact text should exist")
        .contains("Browser Follow"));
    assert!(output["navigationCompactText"]
        .as_str()
        .expect("navigation compact text should exist")
        .contains("Advanced guide"));
    assert_ne!(
        output["compactText"],
        output["navigationCompactText"],
        "session compact output should keep the navigation slice separate from the primary compact surface",
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn replays_browser_trace_into_new_browser_session() {
    let session_file = temp_session_path("browser-replay");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
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
    dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

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

    dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-follow".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

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

    dispatch(CliCommand::Open(TargetOptions {
        target: server.url("/static"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("first browser-backed open should persist session");

    dispatch(CliCommand::Open(TargetOptions {
        target: server.url("/docs-shell"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("second browser-backed open should append session");

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

    dispatch(CliCommand::Open(TargetOptions {
        target: server.url("/polluted-selector"),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");

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
