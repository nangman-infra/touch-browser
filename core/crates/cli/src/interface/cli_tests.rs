use std::{
    fs,
    io::Cursor,
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::json;
use tiny_http::{Header, Response as TinyResponse, Server, StatusCode};
use touch_browser_contracts::{
    ActionName, ReplayTranscript, ReplayTranscriptEntry, RiskClass, SearchReport, SearchResultItem,
    SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument,
    SnapshotEvidence, SnapshotSource, SourceType, TranscriptKind, TranscriptPayloadType,
    CONTRACT_VERSION,
};

use crate::{
    application::search_support::{
        build_search_report, default_search_session_file, derived_search_result_session_file,
    },
    browser_context_dir_for_session_file, build_browser_cli_session, build_cli_error_payload,
    command_usage, dispatch, load_browser_cli_session, parse_command, preprocess_cli_args,
    repo_root, save_browser_cli_session, AckRisk, ApproveOptions, CliCommand, CliError,
    ClickOptions, ExpandOptions, ExtractOptions, FollowOptions, ObservationCompiler,
    ObservationInput, OutputFormat, PaginateOptions, PaginationDirection, PersistedBrowserState,
    PolicyProfile, ReadViewOutput, SearchActionActor, SearchEngine, SearchOpenResultOptions,
    SearchOpenTopOptions, SearchOptions, SearchReportStatus, SessionExtractOptions,
    SessionFileOptions, SessionProfileSetOptions, SessionReadOptions, SessionRefreshOptions,
    SessionSynthesizeOptions, SubmitOptions, TargetOptions, TelemetryRecentOptions, TypeOptions,
    DEFAULT_OPENED_AT, DEFAULT_REQUESTED_TOKENS, DEFAULT_SEARCH_TOKENS,
};

fn temp_session_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be monotonic")
        .as_nanos();
    std::env::temp_dir().join(format!("touch-browser-{name}-{nanos}.json"))
}

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

struct ReplayScenarioFixture {
    scenario: String,
    root: PathBuf,
}

impl ReplayScenarioFixture {
    fn create(name: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos();
        let scenario = format!("{name}-{nanos}");
        let root = repo_root().join("fixtures/scenarios").join(&scenario);
        fs::create_dir_all(&root).expect("replay scenario dir should exist");

        let transcript = ReplayTranscript {
            version: CONTRACT_VERSION.to_string(),
            session_id: "sscenario001".to_string(),
            entries: vec![
                ReplayTranscriptEntry {
                    seq: 1,
                    timestamp: "2026-03-14T00:00:01+09:00".to_string(),
                    kind: TranscriptKind::Command,
                    payload_type: TranscriptPayloadType::ActionCommand,
                    payload: json!({
                        "version": CONTRACT_VERSION,
                        "action": ActionName::Open,
                        "targetUrl": "fixture://research/static-docs/getting-started",
                        "riskClass": RiskClass::Low,
                        "reason": "Open a read-only research document."
                    }),
                },
                ReplayTranscriptEntry {
                    seq: 2,
                    timestamp: "2026-03-14T00:00:02+09:00".to_string(),
                    kind: TranscriptKind::Command,
                    payload_type: TranscriptPayloadType::ActionCommand,
                    payload: json!({
                        "version": CONTRACT_VERSION,
                        "action": ActionName::Follow,
                        "targetRef": "rnav:link:pricing",
                        "riskClass": RiskClass::Low,
                        "reason": "Follow the pricing link in the current snapshot."
                    }),
                },
                ReplayTranscriptEntry {
                    seq: 3,
                    timestamp: "2026-03-14T00:00:03+09:00".to_string(),
                    kind: TranscriptKind::Command,
                    payload_type: TranscriptPayloadType::ActionCommand,
                    payload: json!({
                        "version": CONTRACT_VERSION,
                        "action": ActionName::Extract,
                        "targetUrl": "fixture://research/citation-heavy/pricing",
                        "riskClass": RiskClass::Low,
                        "reason": "Extract supported and unsupported claims.",
                        "input": {
                            "claims": [
                                {
                                    "id": "c1",
                                    "statement": "The Starter plan costs $29 per month."
                                },
                                {
                                    "id": "c2",
                                    "statement": "There is an Enterprise plan."
                                }
                            ]
                        }
                    }),
                },
            ],
        };

        fs::write(
            root.join("replay-transcript.json"),
            format!(
                "{}\n",
                serde_json::to_string_pretty(&transcript)
                    .expect("replay transcript should serialize")
            ),
        )
        .expect("replay transcript should be writable");

        Self { scenario, root }
    }
}

impl Drop for ReplayScenarioFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct CliTestServer {
    base_url: String,
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl CliTestServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener bind");
        let address = listener.local_addr().expect("local addr");
        let server = Server::from_listener(listener, None).expect("server");
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_thread = stop_flag.clone();
        let base_url = format!("http://{}", address);

        let handle = thread::spawn(move || {
            while !stop_flag_thread.load(Ordering::SeqCst) {
                let Ok(Some(request)) = server.recv_timeout(std::time::Duration::from_millis(100))
                else {
                    continue;
                };

                let response = match request.url() {
                    "/robots.txt" => text_response(
                        "User-agent: *\nDisallow:\n",
                        "text/plain; charset=utf-8",
                        200,
                    ),
                    "/static" => html_response(
                        r#"<!doctype html>
                        <html>
                          <head><title>Static Docs</title></head>
                          <body>
                            <main>
                              <h1>Static Docs</h1>
                              <p>Static content works over HTTP without client-side rendering.</p>
                            </main>
                          </body>
                        </html>"#,
                        200,
                    ),
                    "/spa" => html_response(
                        r#"<!doctype html>
                        <html>
                          <head><title>SPA Shell</title></head>
                          <body>
                            <noscript>Please enable JavaScript to run this app.</noscript>
                            <div id="app"></div>
                            <script>
                              document.getElementById('app').innerHTML =
                                '<main><h1>Client Rendered Docs</h1><p>The browser runtime can read JS apps.</p></main>';
                            </script>
                          </body>
                        </html>"#,
                        200,
                    ),
                    "/docs-shell" => html_response(
                        r#"<!doctype html>
                        <html>
                          <head><title>Docs Shell</title></head>
                          <body>
                            <header>
                              <a href="/docs">Docs</a>
                              <a href="/guides">Guides</a>
                              <button>Search</button>
                              <button>Ask AI</button>
                            </header>
                            <aside>
                              <a href="/guides/start">Getting Started</a>
                              <a href="/guides/routing">Routing</a>
                              <a href="/guides/data">Data</a>
                              <a href="/guides/deploy">Deploying</a>
                              <a href="/guides/testing">Testing</a>
                              <a href="/guides/security">Security</a>
                            </aside>
                            <div id="content-area"></div>
                            <script>
                              document.getElementById('content-area').innerHTML =
                                '<main><h1>Rendered Guide</h1><p>The browser runtime can recover shell-heavy docs pages.</p><p>It should auto-select browser capture when HTTP only sees navigation chrome.</p></main>';
                            </script>
                          </body>
                        </html>"#,
                        200,
                    ),
                    "/polluted-selector" => html_response(
                        r#"<!doctype html>
                        <html>
                          <head><title>Node Downloads</title></head>
                          <body>
                            <main>
                              <h1>Node Downloads</h1>
                              <p>
                                Get Node.js v24.14.1 (LTS)
                                <noscript>
                                  <style>.select-hidden { display: none !important; }</style>
                                  <div class="index-module__select">macOS Windows Linux</div>
                                </noscript>
                              </p>
                              <button
                                id="platform-trigger"
                                type="button"
                                aria-label="Platform"
                                aria-haspopup="listbox"
                                aria-expanded="false"
                                aria-controls="platform-options"
                              >
                                Linux
                              </button>
                              <ul id="platform-options" role="listbox" hidden>
                                <li role="option">macOS</li>
                                <li role="option">Windows</li>
                                <li role="option">Linux</li>
                              </ul>
                            </main>
                            <script>
                              const trigger =
                                document.getElementById("platform-trigger");
                              const list =
                                document.getElementById("platform-options");
                              trigger?.addEventListener("click", () => {
                                trigger.setAttribute("aria-expanded", "true");
                                list.hidden = false;
                              });
                            </script>
                          </body>
                        </html>"#,
                        200,
                    ),
                    _ => html_response("<html><body>missing</body></html>", 404),
                };

                let _ = request.respond(response);
            }
        });

        Self {
            base_url,
            stop_flag,
            handle: Some(handle),
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }
}

impl Drop for CliTestServer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn text_response(body: &str, content_type: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", content_type).expect("header");
    TinyResponse::new(
        StatusCode(status),
        vec![header],
        Cursor::new(body.as_bytes().to_vec()),
        Some(body.len()),
        None,
    )
}

fn html_response(body: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    text_response(body, "text/html; charset=utf-8", status)
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
    let snapshot = SnapshotDocument {
        version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
        stable_ref_version: touch_browser_contracts::STABLE_REF_VERSION.to_string(),
        source: SnapshotSource {
            source_url: "https://example.com/read-view".to_string(),
            source_type: SourceType::Http,
            title: Some("Read View".to_string()),
        },
        budget: SnapshotBudget {
            requested_tokens: 128,
            estimated_tokens: 24,
            emitted_tokens: 24,
            truncated: false,
        },
        blocks: vec![
            SnapshotBlock {
                version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
                id: "b1".to_string(),
                kind: SnapshotBlockKind::Link,
                stable_ref: "rnav:link:toc".to_string(),
                role: SnapshotBlockRole::Content,
                text: "Contents".to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://example.com/read-view".to_string(),
                    source_type: SourceType::Http,
                    dom_path_hint: Some("html > body > nav > a".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
            SnapshotBlock {
                version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
                id: "b2".to_string(),
                kind: SnapshotBlockKind::Heading,
                stable_ref: "rmain:heading:title".to_string(),
                role: SnapshotBlockRole::Content,
                text: "Read View".to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://example.com/read-view".to_string(),
                    source_type: SourceType::Http,
                    dom_path_hint: Some("html > body > main > h1".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
            SnapshotBlock {
                version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
                id: "b3".to_string(),
                kind: SnapshotBlockKind::Text,
                stable_ref: "rmain:text:body".to_string(),
                role: SnapshotBlockRole::Content,
                text: "Main article body.".to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://example.com/read-view".to_string(),
                    source_type: SourceType::Http,
                    dom_path_hint: Some("html > body > main > p".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
            SnapshotBlock {
                version: touch_browser_contracts::CONTRACT_VERSION.to_string(),
                id: "b4".to_string(),
                kind: SnapshotBlockKind::Link,
                stable_ref: "rfooter:link:privacy".to_string(),
                role: SnapshotBlockRole::Content,
                text: "Privacy".to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://example.com/read-view".to_string(),
                    source_type: SourceType::Http,
                    dom_path_hint: Some("html > body > footer > a".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
        ],
    };

    let full = ReadViewOutput::new(&snapshot, None, None, false);
    let main = ReadViewOutput::new(&snapshot, None, None, true);

    assert!(!full.markdown_text.contains("Contents"));
    assert!(full.markdown_text.contains("Privacy"));
    assert!(main.markdown_text.contains("Main article body."));
    assert!(!main.markdown_text.contains("Contents"));
    assert!(!main.markdown_text.contains("Privacy"));
    assert!(full.char_count > main.char_count);
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
}

#[test]
fn structures_google_style_search_results_from_snapshot_blocks() {
    let snapshot = SnapshotDocument {
        version: "1.0.0".to_string(),
        stable_ref_version: "1".to_string(),
        source: SnapshotSource {
            source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
            source_type: SourceType::Playwright,
            title: Some("lambda timeout - Google Search".to_string()),
        },
        budget: SnapshotBudget {
            requested_tokens: DEFAULT_SEARCH_TOKENS,
            estimated_tokens: 256,
            emitted_tokens: 256,
            truncated: false,
        },
        blocks: vec![
            SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: SnapshotBlockKind::Link,
                stable_ref: "rmain:link:aws-lambda-quotas".to_string(),
                role: SnapshotBlockRole::Content,
                text: "Lambda quotas".to_string(),
                attributes: std::collections::BTreeMap::from([(
                    "href".to_string(),
                    json!(
                        "https://docs.aws.amazon.com/lambda/latest/dg/gettingstarted-limits.html"
                    ),
                )]),
                evidence: SnapshotEvidence {
                    source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                    source_type: SourceType::Playwright,
                    dom_path_hint: Some("html > body > main > a:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
            SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b2".to_string(),
                kind: SnapshotBlockKind::Text,
                stable_ref: "rmain:text:aws-lambda-quotas-snippet".to_string(),
                role: SnapshotBlockRole::Supporting,
                text: "Function timeout: 900 seconds (15 minutes).".to_string(),
                attributes: Default::default(),
                evidence: SnapshotEvidence {
                    source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                    source_type: SourceType::Playwright,
                    dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
            SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b3".to_string(),
                kind: SnapshotBlockKind::Link,
                stable_ref: "rmain:link:google-help".to_string(),
                role: SnapshotBlockRole::PrimaryNav,
                text: "Google Help".to_string(),
                attributes: std::collections::BTreeMap::from([(
                    "href".to_string(),
                    json!("https://support.google.com/websearch"),
                )]),
                evidence: SnapshotEvidence {
                    source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
                    source_type: SourceType::Playwright,
                    dom_path_hint: Some("html > body > nav > a:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            },
        ],
    };

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
    let snapshot = SnapshotDocument {
        version: "1.0.0".to_string(),
        stable_ref_version: "1".to_string(),
        source: SnapshotSource {
            source_url: "https://search.brave.com/search?q=lambda+timeout".to_string(),
            source_type: SourceType::Playwright,
            title: Some("lambda timeout - Brave Search".to_string()),
        },
        budget: SnapshotBudget {
            requested_tokens: DEFAULT_SEARCH_TOKENS,
            estimated_tokens: 64,
            emitted_tokens: 64,
            truncated: false,
        },
        blocks: Vec::new(),
    };

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
    let snapshot = SnapshotDocument {
        version: "1.0.0".to_string(),
        stable_ref_version: "1".to_string(),
        source: SnapshotSource {
            source_url: "https://www.google.com/search?q=postgresql+mvcc".to_string(),
            source_type: SourceType::Playwright,
            title: Some("postgresql mvcc - Google Search".to_string()),
        },
        budget: SnapshotBudget {
            requested_tokens: DEFAULT_SEARCH_TOKENS,
            estimated_tokens: 64,
            emitted_tokens: 64,
            truncated: false,
        },
        blocks: Vec::new(),
    };
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
    let snapshot = SnapshotDocument {
        version: "1.0.0".to_string(),
        stable_ref_version: "1".to_string(),
        source: SnapshotSource {
            source_url: "https://www.google.com/search?q=lambda+timeout".to_string(),
            source_type: SourceType::Playwright,
            title: Some("Traffic verification".to_string()),
        },
        budget: SnapshotBudget {
            requested_tokens: DEFAULT_SEARCH_TOKENS,
            estimated_tokens: 96,
            emitted_tokens: 96,
            truncated: false,
        },
        blocks: vec![SnapshotBlock {
            version: "1.0.0".to_string(),
            id: "b1".to_string(),
            kind: SnapshotBlockKind::Text,
            stable_ref: "rmain:text:captcha".to_string(),
            role: SnapshotBlockRole::Supporting,
            text: "Google detected unusual traffic and requires reCAPTCHA verification."
                .to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://www.google.com/sorry/index".to_string(),
                source_type: SourceType::Playwright,
                dom_path_hint: Some("html > body > main".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        }],
    };

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

    let mut persisted =
        load_browser_cli_session(&session_file).expect("session should load after open");
    if let Some(context_dir) = persisted.browser_context_dir.as_ref() {
        let context_path = PathBuf::from(context_dir);
        if context_path.exists() {
            fs::remove_dir_all(context_path).expect("managed context dir should clean up");
        }
    }
    persisted.browser_context_dir = None;
    persisted.latest_search = Some(SearchReport {
        version: "1.0.0".to_string(),
        generated_at: DEFAULT_OPENED_AT.to_string(),
        engine: SearchEngine::Google,
        query: "browser pagination".to_string(),
        search_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        final_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        status: SearchReportStatus::Ready,
        status_detail: None,
        result_count: 1,
        results: vec![SearchResultItem {
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
        recommended_result_ranks: vec![1],
        next_action_hints: Vec::new(),
    });
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

    let mut persisted =
        load_browser_cli_session(&session_file).expect("session should load after open");
    if let Some(context_dir) = persisted.browser_context_dir.as_ref() {
        let context_path = PathBuf::from(context_dir);
        if context_path.exists() {
            fs::remove_dir_all(context_path).expect("managed context dir should clean up");
        }
    }
    persisted.browser_context_dir = None;
    persisted.latest_search = Some(SearchReport {
        version: "1.0.0".to_string(),
        generated_at: DEFAULT_OPENED_AT.to_string(),
        engine: SearchEngine::Google,
        query: "browser pagination".to_string(),
        search_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        final_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        status: SearchReportStatus::Ready,
        status_detail: None,
        result_count: 2,
        results: vec![
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
        recommended_result_ranks: vec![2, 1],
        next_action_hints: Vec::new(),
    });
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
    persisted.latest_search = Some(SearchReport {
        version: "1.0.0".to_string(),
        generated_at: DEFAULT_OPENED_AT.to_string(),
        engine: SearchEngine::Google,
        query: "browser pagination".to_string(),
        search_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        final_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        status: SearchReportStatus::Ready,
        status_detail: None,
        result_count: 1,
        results: vec![SearchResultItem {
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
        recommended_result_ranks: vec![1],
        next_action_hints: Vec::new(),
    });
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
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-login-form".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let email_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| {
            block["kind"] == "input"
                && block["text"]
                    .as_str()
                    .expect("input text should exist")
                    .contains("agent@example.com")
        })
        .and_then(|block| block["ref"].as_str())
        .expect("email input ref should exist")
        .to_string();

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

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn rejects_sensitive_type_without_explicit_opt_in() {
    let session_file = temp_session_path("session-type-sensitive");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-login-form".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let password_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| {
            block["kind"] == "input"
                && block["text"]
                    .as_str()
                    .expect("input text should exist")
                    .contains("password")
        })
        .and_then(|block| block["ref"].as_str())
        .expect("password input ref should exist")
        .to_string();

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

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn clicks_browser_session_button_after_interactive_typing() {
    let session_file = temp_session_path("session-click");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-login-form".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let email_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| {
            block["kind"] == "input"
                && block["text"]
                    .as_str()
                    .expect("input text should exist")
                    .contains("agent@example.com")
        })
        .and_then(|block| block["ref"].as_str())
        .expect("email input ref should exist")
        .to_string();
    let button_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "button")
        .and_then(|block| block["ref"].as_str())
        .expect("button ref should exist")
        .to_string();

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

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn submits_browser_session_form_after_interactive_typing() {
    let session_file = temp_session_path("session-submit");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-login-form".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let email_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| {
            block["kind"] == "input"
                && block["text"]
                    .as_str()
                    .expect("input text should exist")
                    .contains("agent@example.com")
        })
        .and_then(|block| block["ref"].as_str())
        .expect("email input ref should exist")
        .to_string();
    let form_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "form")
        .and_then(|block| block["ref"].as_str())
        .expect("form ref should exist")
        .to_string();

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

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn rejects_mfa_submit_without_ack_and_allows_it_with_ack() {
    let session_file = temp_session_path("session-mfa");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-mfa-challenge".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let otp_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "input")
        .and_then(|block| block["ref"].as_str())
        .expect("otp ref should exist")
        .to_string();
    let form_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "form")
        .and_then(|block| block["ref"].as_str())
        .expect("form ref should exist")
        .to_string();

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

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn checkpoint_and_approve_enable_supervised_session_without_repeating_ack_flags() {
    let session_file = temp_session_path("session-checkpoint-approve");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-mfa-challenge".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let otp_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "input")
        .and_then(|block| block["ref"].as_str())
        .expect("otp ref should exist")
        .to_string();
    let form_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "form")
        .and_then(|block| block["ref"].as_str())
        .expect("form ref should exist")
        .to_string();

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

    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

#[test]
fn rejects_high_risk_submit_without_ack_and_allows_it_with_ack() {
    let session_file = temp_session_path("session-high-risk");
    let open_output = dispatch(CliCommand::Open(TargetOptions {
        target: "fixture://research/navigation/browser-high-risk-checkout".to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.clone()),
    }))
    .expect("browser-backed open should persist session");
    let form_ref = open_output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| block["kind"] == "form")
        .and_then(|block| block["ref"].as_str())
        .expect("form ref should exist")
        .to_string();

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
