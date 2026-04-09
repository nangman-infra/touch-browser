use super::*;

pub(super) fn temp_session_path(name: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time should be monotonic")
        .as_nanos();
    std::env::temp_dir().join(format!("touch-browser-{name}-{nanos}.json"))
}

pub(super) fn open_browser_fixture_session(session_file: &Path, target: &str) -> Value {
    dispatch(CliCommand::Open(TargetOptions {
        target: target.to_string(),
        budget: DEFAULT_REQUESTED_TOKENS,
        source_risk: None,
        source_label: None,
        allowlisted_domains: vec!["research".to_string()],
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.to_path_buf()),
    }))
    .expect("browser-backed open should persist session")
}

pub(super) fn open_browser_session(session_file: &Path, target: impl Into<String>) -> Value {
    open_browser_session_with_budget(session_file, target, DEFAULT_REQUESTED_TOKENS)
}

pub(super) fn open_browser_session_with_budget(
    session_file: &Path,
    target: impl Into<String>,
    budget: usize,
) -> Value {
    dispatch(CliCommand::Open(TargetOptions {
        target: target.into(),
        budget,
        source_risk: None,
        source_label: None,
        allowlisted_domains: Vec::new(),
        browser: true,
        headed: false,
        main_only: false,
        session_file: Some(session_file.to_path_buf()),
    }))
    .expect("browser-backed open should persist session")
}

pub(super) fn close_browser_fixture_session(session_file: PathBuf) {
    dispatch(CliCommand::SessionClose(SessionFileOptions {
        session_file,
    }))
    .expect("session close should succeed");
}

pub(super) fn browser_session_block_ref(
    output: &Value,
    kind: &str,
    text_contains: Option<&str>,
) -> String {
    output["output"]["blocks"]
        .as_array()
        .expect("blocks should exist")
        .iter()
        .find(|block| {
            if block["kind"] != kind {
                return false;
            }

            text_contains.is_none_or(|needle| {
                block["text"]
                    .as_str()
                    .expect("block text should exist")
                    .contains(needle)
            })
        })
        .and_then(|block| block["ref"].as_str())
        .expect("matching block ref should exist")
        .to_string()
}

pub(super) fn load_search_browser_session(session_file: &Path, target: &str) -> BrowserCliSession {
    open_browser_fixture_session(session_file, target);
    let mut persisted =
        load_browser_cli_session(session_file).expect("session should load after open");
    if let Some(context_dir) = persisted.browser_context_dir.as_ref() {
        let context_path = PathBuf::from(context_dir);
        if context_path.exists() {
            fs::remove_dir_all(context_path).expect("managed context dir should clean up");
        }
    }
    persisted.browser_context_dir = None;
    persisted
}

pub(super) fn fixture_search_report(
    result_count: usize,
    results: Vec<SearchResultItem>,
    recommended_result_ranks: Vec<usize>,
) -> SearchReport {
    SearchReport {
        version: "1.0.0".to_string(),
        generated_at: DEFAULT_OPENED_AT.to_string(),
        engine: SearchEngine::Google,
        query: "browser pagination".to_string(),
        search_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        final_url: "https://www.google.com/search?q=browser+pagination".to_string(),
        status: SearchReportStatus::Ready,
        status_detail: None,
        result_count,
        results,
        recommended_result_ranks,
        next_action_hints: Vec::new(),
    }
}

pub(super) struct ReplayScenarioFixture {
    pub(super) scenario: String,
    root: PathBuf,
}

impl ReplayScenarioFixture {
    pub(super) fn create(name: &str) -> Self {
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

pub(super) struct CliTestServer {
    base_url: String,
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl CliTestServer {
    pub(super) fn start() -> Self {
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

    pub(super) fn url(&self, path: &str) -> String {
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

pub(super) fn text_response(
    body: &str,
    content_type: &str,
    status: u16,
) -> TinyResponse<Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", content_type).expect("header");
    TinyResponse::new(
        StatusCode(status),
        vec![header],
        Cursor::new(body.as_bytes().to_vec()),
        Some(body.len()),
        None,
    )
}

pub(super) fn html_response(body: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    text_response(body, "text/html; charset=utf-8", status)
}
