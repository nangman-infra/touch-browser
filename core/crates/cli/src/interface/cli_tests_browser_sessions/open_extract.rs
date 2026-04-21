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
    assert_eq!(output["diagnostics"]["captureMode"], "browser");
    assert_eq!(
        output["diagnostics"]["recommendedNextStep"],
        "use-read-view"
    );
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
    assert_eq!(output["diagnostics"]["captureMode"], "browser-fallback");
    assert!(matches!(
        output["diagnostics"]["fallbackReason"].as_str(),
        Some("js-placeholder" | "missing-main-content")
    ));
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
    let command = CliCommand::Extract(ExtractOptions {
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
    });
    let output = dispatch(command.clone()).expect("browser-backed extract should succeed");

    assert_eq!(
        output["open"]["output"]["source"]["sourceType"],
        "playwright"
    );
    assert_eq!(output["extract"]["status"], "succeeded");
    assert_eq!(
        output["extract"]["output"]["evidenceSupportedClaims"][0]["statement"],
        "The Starter plan costs $29 per month."
    );
    let enriched = crate::interface::agent_contract::enrich_output(&command, output);
    assert_eq!(
        enriched["extract"]["output"]["claimOutcomes"][0]["reuseAllowed"],
        true
    );
    assert_eq!(enriched["reuseSummary"]["allClaimsReusable"], true);
    assert_eq!(
        enriched["nextActions"][0]["action"],
        "answer-with-citations"
    );
    assert_eq!(
        enriched["nextActions"][0]["command"],
        serde_json::Value::Null
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
    let _guard = crate::test_env_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
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
