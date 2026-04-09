use std::net::TcpListener;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::{fs, io::Cursor};
use std::{
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

use tiny_http::{Header, Response as TinyResponse, Server, StatusCode};
use touch_browser_contracts::CacheStatus;
use url::Url;

use super::{
    robots, AcquisitionConfig, AcquisitionEngine, AcquisitionError, FixtureResource, RobotsPolicy,
};

#[test]
fn fetches_fixtures_and_caches_by_requested_url() {
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");
    engine.register_fixture(
        "fixture://research/static-docs/getting-started",
        FixtureResource::html("<html><body><h1>Docs</h1></body></html>"),
    );

    let first = engine
        .fetch("fixture://research/static-docs/getting-started")
        .expect("first fetch should work");
    let second = engine
        .fetch("fixture://research/static-docs/getting-started")
        .expect("second fetch should work");

    assert_eq!(first.record.cache_status, CacheStatus::Miss);
    assert_eq!(second.record.cache_status, CacheStatus::Hit);
    assert_eq!(first.body, second.body);
}

#[test]
fn follows_redirects_and_returns_final_document() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests.clone(), false);
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");

    let result = engine
        .fetch(&format!("{}/start", server.base_url()))
        .expect("redirect fetch should work");

    assert_eq!(
        result.record.final_url,
        format!("{}/final", server.base_url())
    );
    assert_eq!(
        result.record.redirect_chain,
        vec![
            format!("{}/start", server.base_url()),
            format!("{}/final", server.base_url())
        ]
    );
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

#[test]
fn blocks_urls_disallowed_by_robots() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests, true);
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");

    let error = engine
        .fetch(&format!("{}/blocked", server.base_url()))
        .expect_err("blocked url should fail");

    assert!(matches!(error, AcquisitionError::BlockedByRobots(_)));
}

#[test]
fn rejects_unsupported_content_types() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests, false);
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");

    let error = engine
        .fetch(&format!("{}/json", server.base_url()))
        .expect_err("json fetch should fail");

    assert!(matches!(error, AcquisitionError::UnsupportedContentType(_)));
}

#[test]
fn returns_cache_hit_without_second_network_fetch() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests.clone(), false);
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");

    let first = engine
        .fetch(&format!("{}/final", server.base_url()))
        .expect("first fetch");
    let second = engine
        .fetch(&format!("{}/final", server.base_url()))
        .expect("second fetch");

    assert_eq!(first.record.cache_status, CacheStatus::Miss);
    assert_eq!(second.record.cache_status, CacheStatus::Hit);
    assert_eq!(requests.load(Ordering::SeqCst), 2);
}

#[test]
fn caches_redirect_targets_by_final_url() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests.clone(), false);
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");

    let redirected = engine
        .fetch(&format!("{}/start", server.base_url()))
        .expect("redirect fetch should work");
    let direct = engine
        .fetch(&format!("{}/final", server.base_url()))
        .expect("final fetch should be served from cache");

    assert_eq!(redirected.record.cache_status, CacheStatus::Miss);
    assert_eq!(direct.record.cache_status, CacheStatus::Hit);
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

#[test]
fn normalizes_fragment_only_urls_into_a_single_cache_key() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests.clone(), false);
    let mut engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");

    let with_fragment = engine
        .fetch(&format!("{}/final#pricing", server.base_url()))
        .expect("fragment fetch should work");
    let normalized = engine
        .fetch(&format!("{}/final", server.base_url()))
        .expect("normalized fetch should hit cache");

    assert_eq!(with_fragment.record.cache_status, CacheStatus::Miss);
    assert_eq!(normalized.record.cache_status, CacheStatus::Hit);
    assert_eq!(with_fragment.record.final_url, normalized.record.final_url);
    assert_eq!(requests.load(Ordering::SeqCst), 2);
}

#[test]
fn reuses_persistent_cache_across_engine_instances() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests.clone(), false);
    let cache_dir = std::env::temp_dir().join(format!(
        "touch-browser-acquisition-cache-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos()
    ));
    let config = AcquisitionConfig {
        persistent_cache_dir: Some(cache_dir.clone()),
        ..AcquisitionConfig::default()
    };

    let first = {
        let mut engine = AcquisitionEngine::new(config.clone()).expect("engine");
        engine
            .fetch(&format!("{}/final", server.base_url()))
            .expect("first fetch")
    };
    let second = {
        let mut engine = AcquisitionEngine::new(config).expect("engine");
        engine
            .fetch(&format!("{}/final", server.base_url()))
            .expect("second fetch")
    };

    assert_eq!(first.record.cache_status, CacheStatus::Miss);
    assert_eq!(second.record.cache_status, CacheStatus::Hit);
    assert_eq!(requests.load(Ordering::SeqCst), 2);

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn falls_back_to_default_policy_when_robots_fetch_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener bind");
    let address = listener.local_addr().expect("local addr");
    drop(listener);

    let engine = AcquisitionEngine::new(AcquisitionConfig::default()).expect("engine");
    let url = Url::parse(&format!("http://{address}/final")).expect("url should parse");

    let policy = robots::fetch_robots_policy(&engine.config, &url, |next_url| engine.get(next_url))
        .expect("robots fetch failures should fall back to default policy");

    assert_eq!(policy, RobotsPolicy::default());
}

#[test]
fn reuses_persistent_robots_cache_across_engine_instances() {
    let requests = Arc::new(AtomicUsize::new(0));
    let server = TestServer::start(requests.clone(), false);
    let cache_dir = std::env::temp_dir().join(format!(
        "touch-browser-robots-cache-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time should be monotonic")
            .as_nanos()
    ));
    let config = AcquisitionConfig {
        persistent_cache_dir: Some(cache_dir.clone()),
        ..AcquisitionConfig::default()
    };

    {
        let mut engine = AcquisitionEngine::new(config.clone()).expect("engine");
        engine
            .fetch(&format!("{}/final", server.base_url()))
            .expect("first fetch");
    }
    requests.store(0, Ordering::SeqCst);
    {
        let mut engine = AcquisitionEngine::new(config).expect("engine");
        engine
            .fetch(&format!("{}/another", server.base_url()))
            .expect("second fetch");
    }

    assert_eq!(
        requests.load(Ordering::SeqCst),
        1,
        "second engine should reuse persisted robots policy and only fetch the document",
    );

    let _ = fs::remove_dir_all(cache_dir);
}

#[test]
fn transport_attempts_retry_connect_errors_and_include_transport_labels() {
    let attempts = [
        super::HttpTransportProfile::RustlsAdaptive,
        super::HttpTransportProfile::RustlsHttp1Only,
        super::HttpTransportProfile::NativeTls,
    ];
    let mut visited = Vec::new();

    let result = super::execute_transport_attempts(
        &attempts,
        |transport| {
            visited.push(transport.label().to_string());
            match transport {
                super::HttpTransportProfile::RustlsAdaptive => Err("error sending request"),
                super::HttpTransportProfile::RustlsHttp1Only => Err("connection reset"),
                super::HttpTransportProfile::NativeTls => Ok("ok"),
            }
        },
        |error| error.contains("request") || error.contains("connection"),
    )
    .expect("native tls fallback should succeed");

    assert_eq!(result, "ok");
    assert_eq!(visited, vec!["rustls", "rustls-http1", "native-tls"]);
}

#[test]
fn transport_attempts_stop_after_non_retryable_error() {
    let attempts = [
        super::HttpTransportProfile::RustlsAdaptive,
        super::HttpTransportProfile::RustlsHttp1Only,
        super::HttpTransportProfile::NativeTls,
    ];
    let mut visited = Vec::new();

    let failures = super::execute_transport_attempts(
        &attempts,
        |transport| {
            visited.push(transport.label().to_string());
            Err::<&'static str, &'static str>("unsupported content type")
        },
        |error| error.contains("connection"),
    )
    .expect_err("non-retryable errors should stop immediately");

    assert_eq!(visited, vec!["rustls"]);
    assert_eq!(
        super::format_transport_failures(&failures),
        "rustls: unsupported content type"
    );
}

struct TestServer {
    base_url: String,
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl TestServer {
    fn start(requests: Arc<AtomicUsize>, robots_blocks: bool) -> Self {
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
                requests.fetch_add(1, Ordering::SeqCst);
                let path = request.url().to_string();

                let response = match path.as_str() {
                    "/robots.txt" => {
                        let body = if robots_blocks {
                            "User-agent: *\nDisallow: /blocked\n"
                        } else {
                            "User-agent: *\nDisallow:\n"
                        };
                        html_response(body, "text/plain; charset=utf-8", 200)
                    }
                    "/start" => redirect_response("/final"),
                    "/final" => html_response(
                        "<html><body><h1>Final</h1></body></html>",
                        "text/html; charset=utf-8",
                        200,
                    ),
                    "/another" => html_response(
                        "<html><body><h1>Another</h1></body></html>",
                        "text/html; charset=utf-8",
                        200,
                    ),
                    "/blocked" => html_response(
                        "<html><body><h1>Blocked</h1></body></html>",
                        "text/html; charset=utf-8",
                        200,
                    ),
                    "/json" => html_response("{\"ok\":true}", "application/json", 200),
                    _ => html_response("<html><body>missing</body></html>", "text/html", 404),
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

    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn html_response(body: &str, content_type: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", content_type).expect("header");
    TinyResponse::new(
        StatusCode(status),
        vec![header],
        Cursor::new(body.as_bytes().to_vec()),
        Some(body.len()),
        None,
    )
}

fn redirect_response(location: &str) -> TinyResponse<Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Location", location).expect("location");
    TinyResponse::new(
        StatusCode(302),
        vec![header],
        Cursor::new(Vec::new()),
        Some(0),
        None,
    )
}
