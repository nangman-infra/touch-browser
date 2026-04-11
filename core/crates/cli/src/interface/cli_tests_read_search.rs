use super::*;

#[test]
fn dispatches_read_view_for_fixture_target() {
    let output = dispatch(CliCommand::ReadView(TargetOptions {
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
    .expect("read-view should succeed");

    let markdown = output["markdownText"]
        .as_str()
        .expect("markdown text should be present");
    assert!(markdown.starts_with('#'));
    assert!(markdown.contains("Getting Started"));
    assert!(output["approxTokens"].as_u64().unwrap_or(0) > 0);
}

#[test]
fn read_view_output_changes_when_main_only_is_enabled() {
    let source_url = "https://example.com/read-view";
    let snapshot = test_snapshot_document(
        source_url,
        SourceType::Http,
        "Read View",
        128,
        24,
        vec![
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b1",
                SnapshotBlockKind::Link,
                "rnav:link:toc",
                SnapshotBlockRole::Content,
                "Contents",
                "html > body > nav > a",
            ),
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b2",
                SnapshotBlockKind::Heading,
                "rmain:heading:title",
                SnapshotBlockRole::Content,
                "Read View",
                "html > body > main > h1",
            ),
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b3",
                SnapshotBlockKind::Text,
                "rmain:text:body",
                SnapshotBlockRole::Content,
                "Main article body.",
                "html > body > main > p",
            ),
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b4",
                SnapshotBlockKind::Link,
                "rfooter:link:privacy",
                SnapshotBlockRole::Content,
                "Privacy",
                "html > body > footer > a",
            ),
        ],
    );

    let full = ReadViewOutput::new(&snapshot, None, None, false);
    let main = ReadViewOutput::new(&snapshot, None, None, true);

    assert!(!full.markdown_text.contains("Contents"));
    assert!(full.markdown_text.contains("Privacy"));
    assert!(main.markdown_text.contains("Main article body."));
    assert!(!main.markdown_text.contains("Contents"));
    assert!(!main.markdown_text.contains("Privacy"));
    assert!(full.char_count > main.char_count);
    assert_eq!(
        main.main_content_quality.as_deref(),
        Some("high"),
        "expected article-like main content to be trusted"
    );
}

#[test]
fn read_view_main_only_filters_wikipedia_language_header_noise() {
    let snapshot = ObservationCompiler
        .compile(&ObservationInput::new(
            "https://zh.wikipedia.org/wiki/%E4%B8%AD%E5%9B%BD",
            SourceType::Http,
            r##"
                <html>
                  <body>
                    <main>
                      <header class="mw-body-header vector-page-titlebar">
                        <h1 id="firstHeading">中國</h1>
                        <div id="vector-page-titlebar-toc"><a href="#history">目录</a></div>
                        <div id="p-lang-btn">
                          <ul>
                            <li><a class="interlanguage-link-target" href="https://en.wikipedia.org/wiki/China">English</a></li>
                          </ul>
                        </div>
                      </header>
                      <div id="mw-content-text">
                        <div class="mw-parser-output">
                          <p>中國位於東亞。</p>
                        </div>
                      </div>
                    </main>
                  </body>
                </html>
            "##,
            512,
        ))
        .expect("observation should compile");

    let main = ReadViewOutput::new(&snapshot, None, None, true);
    assert!(main.markdown_text.contains("# 中國"));
    assert!(main.markdown_text.contains("中國位於東亞。"));
    assert!(!main.markdown_text.contains("English"));
    assert!(!main.markdown_text.contains("目录"));
    assert_eq!(main.main_content_quality.as_deref(), Some("high"));
}

#[test]
fn read_view_main_only_reports_poor_quality_for_navigation_heavy_output() {
    let source_url = "https://example.com/noisy-shell";
    let snapshot = test_snapshot_document(
        source_url,
        SourceType::Http,
        "Noisy Shell",
        128,
        24,
        vec![
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b1",
                SnapshotBlockKind::Link,
                "rmain:link:docs",
                SnapshotBlockRole::Content,
                "Docs",
                "html > body > main > nav > a",
            ),
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b2",
                SnapshotBlockKind::Link,
                "rmain:link:pricing",
                SnapshotBlockRole::Content,
                "Pricing",
                "html > body > main > nav > a:nth-of-type(2)",
            ),
            test_snapshot_block(
                source_url,
                SourceType::Http,
                "b3",
                SnapshotBlockKind::Text,
                "rmain:text:cta",
                SnapshotBlockRole::Content,
                "Start free",
                "html > body > main > div.hero > p",
            ),
        ],
    );

    let main = ReadViewOutput::new(&snapshot, None, None, true);

    assert_eq!(main.main_content_quality.as_deref(), Some("poor"));
    assert!(
        main.main_content_hint
            .as_deref()
            .is_some_and(|hint| hint.contains("noisy")),
        "expected noisy-shell guidance"
    );
}

#[test]
fn structures_google_style_search_results_from_snapshot_blocks() {
    let source_url = "https://www.google.com/search?q=lambda+timeout";
    let snapshot = test_snapshot_document(
        source_url,
        SourceType::Playwright,
        "lambda timeout - Google Search",
        DEFAULT_SEARCH_TOKENS,
        256,
        vec![
            test_snapshot_block_with_attributes(
                source_url,
                SourceType::Playwright,
                "b1",
                SnapshotBlockKind::Link,
                "rmain:link:aws-lambda-quotas",
                SnapshotBlockRole::Content,
                "Lambda quotas",
                "html > body > main > a:nth-of-type(1)",
                std::collections::BTreeMap::from([(
                    "href".to_string(),
                    json!(
                        "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html"
                    ),
                )]),
            ),
            test_snapshot_block(
                source_url,
                SourceType::Playwright,
                "b2",
                SnapshotBlockKind::Text,
                "rmain:text:aws-lambda-quotas-snippet",
                SnapshotBlockRole::Supporting,
                "Function timeout: 900 seconds (15 minutes).",
                "html > body > main > p:nth-of-type(1)",
            ),
            test_snapshot_block_with_attributes(
                source_url,
                SourceType::Playwright,
                "b3",
                SnapshotBlockKind::Link,
                "rmain:link:google-help",
                SnapshotBlockRole::PrimaryNav,
                "Google Help",
                "html > body > nav > a:nth-of-type(1)",
                std::collections::BTreeMap::from([(
                    "href".to_string(),
                    json!("https://support.google.com/websearch"),
                )]),
            ),
        ],
    );

    let report = build_search_report(
        SearchEngine::Google,
        "lambda timeout",
        "https://www.google.com/search?q=lambda+timeout",
        &snapshot,
        "<html></html>",
        "https://www.google.com/search?q=lambda+timeout",
        "2026-04-05T00:00:00+09:00",
    );

    assert_eq!(report.status, SearchReportStatus::Ready);
    assert_eq!(report.result_count, 1);
    assert_eq!(report.results[0].rank, 1);
    assert_eq!(report.results[0].domain, "docs.aws.amazon.com".to_string());
    assert_eq!(
        report.results[0].recommended_surface.as_deref(),
        Some("extract")
    );
    assert!(report.next_action_hints.iter().any(|hint| {
        hint.action == "open-top" && hint.actor == SearchActionActor::Ai && hint.can_auto_run
    }));
}

#[test]
fn structures_search_results_from_html_when_snapshot_is_sparse() {
    let snapshot = test_snapshot_document(
        "https://search.brave.com/search?q=lambda+timeout",
        SourceType::Playwright,
        "lambda timeout - Brave Search",
        DEFAULT_SEARCH_TOKENS,
        64,
        Vec::new(),
    );

    let html = r#"
        <html>
          <body>
            <main>
              <div class="snippet" data-type="web">
                <a href="https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html">
                  Lambda quotas
                </a>
                <p>Function timeout: 900 seconds (15 minutes).</p>
              </div>
            </main>
          </body>
        </html>
    "#;

    let report = build_search_report(
        SearchEngine::Brave,
        "lambda timeout",
        "https://search.brave.com/search?q=lambda+timeout",
        &snapshot,
        html,
        "https://search.brave.com/search?q=lambda+timeout",
        "2026-04-05T00:00:00+09:00",
    );

    assert_eq!(report.status, SearchReportStatus::Ready);
    assert_eq!(report.result_count, 1);
    assert_eq!(report.results[0].title, "Lambda quotas");
    assert_eq!(
        report.results[0].snippet.as_deref(),
        Some("Function timeout: 900 seconds (15 minutes).")
    );
}

#[test]
fn deduplicates_youtube_timestamp_variants_in_search_results() {
    let snapshot = test_snapshot_document(
        "https://www.google.com/search?q=postgresql+mvcc",
        SourceType::Playwright,
        "postgresql mvcc - Google Search",
        DEFAULT_SEARCH_TOKENS,
        64,
        Vec::new(),
    );
    let html = r#"
        <html>
          <body>
            <main>
              <div>
                <a href="https://www.youtube.com/watch?v=abc123&t=31s">PostgreSQL MVCC chapter 1</a>
                <p>Explain the first checkpoint.</p>
              </div>
              <div>
                <a href="https://www.youtube.com/watch?v=abc123&t=92s">PostgreSQL MVCC chapter 2</a>
                <p>Explain the second checkpoint.</p>
              </div>
              <div>
                <a href="https://www.postgresql.org/docs/current/mvcc-intro.html">PostgreSQL MVCC docs</a>
                <p>Official documentation.</p>
              </div>
            </main>
          </body>
        </html>
    "#;

    let report = build_search_report(
        SearchEngine::Google,
        "PostgreSQL MVCC",
        "https://www.google.com/search?q=postgresql+mvcc",
        &snapshot,
        html,
        "https://www.google.com/search?q=postgresql+mvcc",
        DEFAULT_OPENED_AT,
    );

    assert_eq!(report.result_count, 2);
    assert_eq!(
        report.results[0].url,
        "https://www.youtube.com/watch?v=abc123"
    );
    assert_eq!(report.results[1].domain, "www.postgresql.org");
}

#[test]
fn marks_google_sorry_pages_as_search_challenges() {
    let snapshot = test_snapshot_document(
        "https://www.google.com/search?q=lambda+timeout",
        SourceType::Playwright,
        "Traffic verification",
        DEFAULT_SEARCH_TOKENS,
        96,
        vec![test_snapshot_block(
            "https://www.google.com/sorry/index",
            SourceType::Playwright,
            "b1",
            SnapshotBlockKind::Text,
            "rmain:text:captcha",
            SnapshotBlockRole::Supporting,
            "Google detected unusual traffic and requires reCAPTCHA verification.",
            "html > body > main",
        )],
    );

    let report = build_search_report(
        SearchEngine::Google,
        "lambda timeout",
        "https://www.google.com/search?q=lambda+timeout",
        &snapshot,
        "<html></html>",
        "https://www.google.com/sorry/index?q=test",
        "2026-04-05T00:00:00+09:00",
    );

    assert_eq!(report.status, SearchReportStatus::Challenge);
    assert_eq!(report.result_count, 0);
    assert!(report.next_action_hints.iter().any(|hint| {
        hint.action == "complete-challenge"
            && hint.actor == SearchActionActor::Human
            && hint.headed_required
            && !hint.can_auto_run
    }));
}

#[test]
fn search_open_result_preserves_latest_search_state() {
    let session_file = temp_session_path("search-open-result-preserve");
    let mut persisted = load_search_browser_session(
        &session_file,
        "fixture://research/navigation/browser-pagination",
    );
    persisted.latest_search = Some(fixture_search_report(
        1,
        vec![SearchResultItem {
            rank: 1,
            title: "Browser Pagination".to_string(),
            url: "fixture://research/navigation/browser-pagination".to_string(),
            domain: "fixture.local".to_string(),
            snippet: Some("Fixture search result".to_string()),
            stable_ref: None,
            official_likely: true,
            selection_score: Some(1.0),
            recommended_surface: Some("read-view".to_string()),
        }],
        vec![1],
    ));
    save_browser_cli_session(&session_file, &persisted)
        .expect("session should save with search state");

    let output = dispatch(CliCommand::SearchOpenResult(SearchOpenResultOptions {
        engine: SearchEngine::Google,
        session_file: Some(session_file.clone()),
        rank: 1,
        prefer_official: false,
        headed: false,
    }))
    .expect("search-open-result should succeed");
    assert_eq!(output["sessionFile"], session_file.display().to_string());
    assert!(output["nextCommands"]["sessionExtract"]
        .as_str()
        .expect("session extract hint should exist")
        .contains("touch-browser session-extract"));
    assert_eq!(output["diagnostics"]["captureMode"], "browser");
    assert_eq!(
        output["result"]["diagnostics"]["requestedBudget"],
        json!(512)
    );

    let refreshed =
        load_browser_cli_session(&session_file).expect("session should reload after open");
    let latest_search = refreshed
        .latest_search
        .expect("latest search should still be present after opening a result");
    assert_eq!(latest_search.result_count, 1);
    assert_eq!(latest_search.results[0].rank, 1);

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("session close should clean search session");
}

#[test]
fn search_open_result_can_prefer_official_candidates() {
    let session_file = temp_session_path("search-open-result-prefer-official");
    let mut persisted = load_search_browser_session(
        &session_file,
        "fixture://research/navigation/browser-pagination",
    );
    persisted.latest_search = Some(fixture_search_report(
        2,
        vec![
            SearchResultItem {
                rank: 1,
                title: "Video summary".to_string(),
                url: "fixture://research/navigation/browser-follow".to_string(),
                domain: "video.example".to_string(),
                snippet: Some("Video result".to_string()),
                stable_ref: None,
                official_likely: false,
                selection_score: Some(0.3),
                recommended_surface: Some("read-view".to_string()),
            },
            SearchResultItem {
                rank: 2,
                title: "Official docs".to_string(),
                url: "fixture://research/navigation/browser-pagination".to_string(),
                domain: "docs.example".to_string(),
                snippet: Some("Official result".to_string()),
                stable_ref: None,
                official_likely: true,
                selection_score: Some(0.9),
                recommended_surface: Some("extract".to_string()),
            },
        ],
        vec![2, 1],
    ));
    save_browser_cli_session(&session_file, &persisted)
        .expect("session should save with search state");

    let output = dispatch(CliCommand::SearchOpenResult(SearchOpenResultOptions {
        engine: SearchEngine::Google,
        session_file: Some(session_file.clone()),
        rank: 1,
        prefer_official: true,
        headed: false,
    }))
    .expect("prefer-official search-open-result should succeed");

    assert_eq!(output["selectionStrategy"], "prefer-official");
    assert_eq!(output["selectedResult"]["rank"], 2);
    assert_eq!(output["selectedResult"]["title"], "Official docs");

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn search_open_top_inherits_external_profile_directory() {
    let session_file = temp_session_path("search-open-top-profile");
    let profile_dir = std::env::temp_dir().join(format!(
        "touch-browser-external-profile-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos()
    ));
    fs::create_dir_all(&profile_dir).expect("external profile dir should exist");

    let mut persisted = load_search_browser_session(
        &session_file,
        "fixture://research/navigation/browser-pagination",
    );
    persisted.browser_profile_dir = Some(profile_dir.display().to_string());
    persisted.latest_search = Some(fixture_search_report(
        1,
        vec![SearchResultItem {
            rank: 1,
            title: "Browser Pagination".to_string(),
            url: "fixture://research/navigation/browser-pagination".to_string(),
            domain: "fixture.local".to_string(),
            snippet: Some("Fixture search result".to_string()),
            stable_ref: None,
            official_likely: true,
            selection_score: Some(1.0),
            recommended_surface: Some("read-view".to_string()),
        }],
        vec![1],
    ));
    save_browser_cli_session(&session_file, &persisted)
        .expect("session should save with external profile");

    dispatch(CliCommand::SearchOpenTop(SearchOpenTopOptions {
        engine: SearchEngine::Google,
        session_file: Some(session_file.clone()),
        limit: 1,
        headed: false,
    }))
    .expect("search-open-top should succeed");

    let result_session_file = derived_search_result_session_file(&session_file, 1);
    let result_session = load_browser_cli_session(&result_session_file)
        .expect("child session should load after open-top");
    assert_eq!(
        result_session.browser_profile_dir.as_deref(),
        Some(profile_dir.to_string_lossy().as_ref())
    );
    assert_eq!(result_session.browser_context_dir, None);
    let output = dispatch(CliCommand::SearchOpenTop(SearchOpenTopOptions {
        engine: SearchEngine::Google,
        session_file: Some(session_file.clone()),
        limit: 1,
        headed: false,
    }))
    .expect("search-open-top should return diagnostics");
    assert_eq!(
        output["opened"][0]["diagnostics"]["requestedBudget"],
        json!(512)
    );

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: result_session_file.clone(),
    }))
    .expect("child session close should succeed");
    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file: session_file.clone(),
    }))
    .expect("search session close should succeed");
    fs::remove_dir_all(&profile_dir).expect("external profile dir cleanup should succeed");
}

#[test]
fn dispatches_fixture_open_with_policy() {
    let output = dispatch(CliCommand::Open(TargetOptions {
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
    .expect("open should succeed");

    assert_eq!(output["status"], "succeeded");
    assert_eq!(output["policy"]["decision"], "allow");
    assert_eq!(output["payloadType"], "snapshot-document");
}

#[test]
fn dispatches_hostile_policy_command() {
    let output = dispatch(CliCommand::Policy(TargetOptions {
        target: "fixture://research/hostile/fake-system-message".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: false,
        headed: false,
        main_only: false,
        session_file: None,
    }))
    .expect("policy command should succeed");

    assert_eq!(output["policy"]["decision"], "block");
    assert_eq!(output["policy"]["riskClass"], "blocked");
}

#[test]
fn dispatches_replay_command() {
    let scenario = ReplayScenarioFixture::create("dispatches-replay-command");
    let output = dispatch(CliCommand::Replay {
        scenario: scenario.scenario.clone(),
    })
    .expect("replay should succeed");

    assert_eq!(output["snapshotCount"], 2);
    assert_eq!(output["evidenceReportCount"], 1);
}

#[test]
fn dispatches_memory_summary_for_fifty_actions() {
    let output =
        dispatch(CliCommand::MemorySummary { steps: 50 }).expect("memory summary should succeed");

    assert_eq!(output["requestedActions"], 50);
    assert_eq!(output["memorySummary"]["turnCount"], 50);
    assert!(
        output["memorySummary"]["maxWorkingSetSize"]
            .as_u64()
            .expect("working set size should be numeric")
            <= 6
    );
}
