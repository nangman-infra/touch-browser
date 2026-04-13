use std::{
    io::Cursor,
    net::TcpListener,
    path::{Path, PathBuf},
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use serde_json::Value;
use tiny_http::{Header, Response as TinyResponse, Server, StatusCode};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .expect("repo root should resolve from cli crate")
        .to_path_buf()
}

fn run_cli(args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_touch-browser"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .expect("touch-browser process should launch")
}

fn temp_missing_session_path() -> PathBuf {
    std::env::temp_dir().join(format!(
        "touch-browser-missing-session-{}.json",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be monotonic")
            .as_nanos()
    ))
}

struct LiveCliServer {
    base_url: String,
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl LiveCliServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener bind");
        let address = listener.local_addr().expect("listener addr");
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
                    "/robots.txt" => text_response("User-agent: *\nDisallow:\n", 200),
                    "/" => html_response(
                        r#"<!doctype html>
                        <html>
                          <head><title>Local Example</title></head>
                          <body>
                            <main>
                              <h1>Local Example</h1>
                              <p>Example value is 42.</p>
                            </main>
                          </body>
                        </html>"#,
                        200,
                    ),
                    "/nav-heavy" => html_response(
                        r#"<!doctype html>
                        <html>
                          <head><title>Hub Page</title></head>
                          <body>
                            <header>
                              <nav>
                                <a href="/cloud">Cloud</a>
                                <a href="/linux">Linux</a>
                                <a href="/containers">Containers</a>
                              </nav>
                            </header>
                            <aside>
                              <a href="/latest">Latest</a>
                              <a href="/guides">Guides</a>
                            </aside>
                            <footer>
                              <a href="/about">About</a>
                              <a href="/contact">Contact</a>
                            </footer>
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

impl Drop for LiveCliServer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn html_response(body: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    TinyResponse::new(
        StatusCode(status),
        vec![Header::from_bytes(
            b"Content-Type".to_vec(),
            b"text/html; charset=utf-8".to_vec(),
        )
        .expect("html content-type header should build")],
        Cursor::new(body.as_bytes().to_vec()),
        Some(body.len()),
        None,
    )
}

fn text_response(body: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    TinyResponse::new(
        StatusCode(status),
        vec![Header::from_bytes(
            b"Content-Type".to_vec(),
            b"text/plain; charset=utf-8".to_vec(),
        )
        .expect("text content-type header should build")],
        Cursor::new(body.as_bytes().to_vec()),
        Some(body.len()),
        None,
    )
}

#[test]
fn parse_errors_write_only_to_stderr() {
    let output = run_cli(&["search", "lambda timeout", "--engine", "invalid-engine"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("Unknown search engine"));
}

#[test]
fn rank_validation_errors_write_only_to_stderr() {
    let output = run_cli(&["search-open-result", "--engine", "google", "--rank", "0"]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--rank requires a positive number."));
}

#[test]
fn runtime_errors_write_only_to_stderr() {
    let missing_session = temp_missing_session_path();
    let output = run_cli(&[
        "session-read",
        "--session-file",
        missing_session
            .to_str()
            .expect("missing session path should be valid UTF-8"),
    ]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).trim().is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains(
        missing_session
            .to_str()
            .expect("missing session path should be valid UTF-8")
    ));
}

#[test]
fn successful_json_output_stays_on_stdout() {
    let output = run_cli(&["open", "fixture://research/static-docs/getting-started"]);

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).trim().is_empty());
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("\"payloadType\": \"snapshot-document\"")
    );
}

#[test]
fn poor_main_only_read_view_emits_quality_note_on_stderr() {
    let server = LiveCliServer::start();
    let output = run_cli(&["read-view", &server.url("/nav-heavy")]);

    assert!(output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("touch-browser note ["),
        "expected stderr quality note, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn live_extract_uses_real_timestamps_instead_of_fixture_defaults() {
    let server = LiveCliServer::start();
    let output = run_cli(&[
        "extract",
        &server.url("/"),
        "--claim",
        "Example value is 42.",
    ]);

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let payload: Value =
        serde_json::from_slice(&output.stdout).expect("extract output should be valid JSON");

    assert_ne!(
        payload["open"]["output"]["source"]["sourceUrl"].as_str(),
        Some(""),
        "sanity check: expected a live source URL",
    );
    assert_ne!(
        payload["extract"]["output"]["generatedAt"].as_str(),
        Some("2026-03-14T00:01:30+09:00"),
        "live extract should not reuse fixture generatedAt",
    );
    assert_ne!(
        payload["sessionState"]["openedAt"].as_str(),
        Some("2026-03-14T00:00:00+09:00"),
        "live session should not reuse fixture openedAt",
    );
    assert_ne!(
        payload["sessionState"]["updatedAt"].as_str(),
        Some("2026-03-14T00:01:30+09:00"),
        "live session should not reuse fixture updatedAt",
    );
}
