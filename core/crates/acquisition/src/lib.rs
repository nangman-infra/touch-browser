use std::{
    collections::{hash_map::DefaultHasher, BTreeMap, HashMap},
    fs,
    hash::{Hash, Hasher},
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_TYPE, LOCATION, USER_AGENT};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use touch_browser_contracts::{AcquisitionRecord, CacheStatus, SourceType, CONTRACT_VERSION};
use url::Url;

pub fn crate_status() -> &'static str {
    "acquisition ready"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionConfig {
    pub user_agent: String,
    pub max_redirects: usize,
    pub persistent_cache_dir: Option<PathBuf>,
    pub persistent_cache_ttl: Duration,
}

impl Default for AcquisitionConfig {
    fn default() -> Self {
        Self {
            user_agent: "TouchBrowser/0.1".to_string(),
            max_redirects: 5,
            persistent_cache_dir: Some(
                std::env::temp_dir().join("touch-browser-acquisition-cache"),
            ),
            persistent_cache_ttl: Duration::from_secs(60),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixtureResource {
    pub content_type: String,
    pub body: String,
}

impl FixtureResource {
    pub fn html(body: impl Into<String>) -> Self {
        Self {
            content_type: "text/html; charset=utf-8".to_string(),
            body: body.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AcquiredDocument {
    pub record: AcquisitionRecord,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CachedDocument {
    result: AcquiredDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentCachedDocument {
    cache_key: String,
    fetched_at_unix_seconds: u64,
    result: AcquiredDocument,
}

#[derive(Debug, Clone, Default)]
struct RobotsPolicy {
    disallow_rules: Vec<String>,
}

pub struct AcquisitionEngine {
    config: AcquisitionConfig,
    client: Client,
    fixtures: BTreeMap<String, FixtureResource>,
    cache: HashMap<String, CachedDocument>,
    robots_cache: HashMap<String, RobotsPolicy>,
}

impl AcquisitionEngine {
    pub fn new(config: AcquisitionConfig) -> Result<Self, AcquisitionError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(AcquisitionError::Http)?;

        Ok(Self {
            config,
            client,
            fixtures: BTreeMap::new(),
            cache: HashMap::new(),
            robots_cache: HashMap::new(),
        })
    }

    pub fn register_fixture(&mut self, url: impl Into<String>, resource: FixtureResource) {
        self.fixtures.insert(url.into(), resource);
    }

    pub fn fetch(&mut self, url: &str) -> Result<AcquiredDocument, AcquisitionError> {
        let cache_key = normalize_cache_key(url)?;
        let persistent_cache_allowed = !url.starts_with("fixture://");

        if let Some(cached) = self.cache.get(&cache_key) {
            let mut result = cached.result.clone();
            result.record.cache_status = CacheStatus::Hit;
            return Ok(result);
        }

        if persistent_cache_allowed {
            if let Some(cached) = self.load_persistent_cache(&cache_key) {
                self.cache.insert(
                    cache_key.clone(),
                    CachedDocument {
                        result: cached.clone(),
                    },
                );
                self.cache.insert(
                    cached.record.final_url.clone(),
                    CachedDocument {
                        result: cached.clone(),
                    },
                );
                return Ok(cached);
            }
        }

        let result = if url.starts_with("fixture://") {
            self.fetch_fixture(url)?
        } else {
            self.fetch_http(&cache_key)?
        };
        self.cache.insert(
            cache_key.clone(),
            CachedDocument {
                result: result.clone(),
            },
        );
        self.cache.insert(
            result.record.final_url.clone(),
            CachedDocument {
                result: result.clone(),
            },
        );
        if persistent_cache_allowed {
            let _ = self.store_persistent_cache(&cache_key, &result);
            let _ = self.store_persistent_cache(&result.record.final_url.clone(), &result);
        }

        Ok(result)
    }

    fn load_persistent_cache(&self, cache_key: &str) -> Option<AcquiredDocument> {
        let cache_path = self.persistent_cache_path(cache_key)?;
        let raw = fs::read(&cache_path).ok()?;
        let cached: PersistentCachedDocument = serde_json::from_slice(&raw).ok()?;
        if cached.cache_key != cache_key {
            return None;
        }
        if self.cache_entry_expired(cached.fetched_at_unix_seconds) {
            let _ = fs::remove_file(cache_path);
            return None;
        }

        let mut result = cached.result;
        result.record.cache_status = CacheStatus::Hit;
        Some(result)
    }

    fn store_persistent_cache(
        &self,
        cache_key: &str,
        result: &AcquiredDocument,
    ) -> Result<(), AcquisitionError> {
        let Some(cache_path) = self.persistent_cache_path(cache_key) else {
            return Ok(());
        };
        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent).map_err(AcquisitionError::PersistentCacheIo)?;
        }

        let cached = PersistentCachedDocument {
            cache_key: cache_key.to_string(),
            fetched_at_unix_seconds: current_unix_seconds(),
            result: result.clone(),
        };
        fs::write(
            cache_path,
            serde_json::to_vec(&cached).map_err(AcquisitionError::PersistentCacheJson)?,
        )
        .map_err(AcquisitionError::PersistentCacheIo)?;
        Ok(())
    }

    fn persistent_cache_path(&self, cache_key: &str) -> Option<PathBuf> {
        let cache_dir = self.config.persistent_cache_dir.as_ref()?;
        Some(cache_dir.join(format!("{:016x}.json", cache_key_hash(cache_key))))
    }

    fn cache_entry_expired(&self, fetched_at_unix_seconds: u64) -> bool {
        self.config.persistent_cache_ttl.is_zero()
            || current_unix_seconds().saturating_sub(fetched_at_unix_seconds)
                > self.config.persistent_cache_ttl.as_secs()
    }

    fn fetch_fixture(&self, url: &str) -> Result<AcquiredDocument, AcquisitionError> {
        let fixture = self
            .fixtures
            .get(url)
            .ok_or_else(|| AcquisitionError::UnknownFixture(url.to_string()))?;

        validate_content_type(&fixture.content_type)?;

        Ok(AcquiredDocument {
            record: AcquisitionRecord {
                version: CONTRACT_VERSION.to_string(),
                requested_url: url.to_string(),
                final_url: url.to_string(),
                source_type: SourceType::Fixture,
                status_code: 200,
                content_type: fixture.content_type.clone(),
                redirect_chain: vec![url.to_string()],
                cache_status: CacheStatus::Miss,
            },
            body: fixture.body.clone(),
        })
    }

    fn fetch_http(&mut self, requested_url: &str) -> Result<AcquiredDocument, AcquisitionError> {
        let mut current_url = Url::parse(requested_url).map_err(AcquisitionError::Url)?;
        self.assert_allowed_by_robots(&current_url)?;

        let mut redirect_chain = vec![current_url.to_string()];
        let mut redirects_followed = 0usize;

        loop {
            let response = self.get(current_url.as_str())?;

            if response.status().is_redirection() {
                if redirects_followed >= self.config.max_redirects {
                    return Err(AcquisitionError::TooManyRedirects(
                        requested_url.to_string(),
                    ));
                }

                let next_url = resolve_redirect(&current_url, &response)?;
                redirects_followed += 1;
                current_url = next_url;
                self.assert_allowed_by_robots(&current_url)?;
                redirect_chain.push(current_url.to_string());
                continue;
            }

            let status_code = response.status().as_u16();
            let content_type = response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_string();
            validate_content_type(&content_type)?;
            let body = response.text().map_err(AcquisitionError::Http)?;

            return Ok(AcquiredDocument {
                record: AcquisitionRecord {
                    version: CONTRACT_VERSION.to_string(),
                    requested_url: requested_url.to_string(),
                    final_url: current_url.to_string(),
                    source_type: SourceType::Http,
                    status_code,
                    content_type,
                    redirect_chain,
                    cache_status: CacheStatus::Miss,
                },
                body,
            });
        }
    }

    fn get(&self, url: &str) -> Result<Response, AcquisitionError> {
        self.client
            .get(url)
            .header(USER_AGENT, self.config.user_agent.as_str())
            .send()
            .map_err(AcquisitionError::Http)
    }

    fn assert_allowed_by_robots(&mut self, url: &Url) -> Result<(), AcquisitionError> {
        let Some(origin) = origin_key(url) else {
            return Ok(());
        };

        if !self.robots_cache.contains_key(&origin) {
            let policy = self.fetch_robots_policy(url)?;
            self.robots_cache.insert(origin.clone(), policy);
        }

        let policy = self
            .robots_cache
            .get(&origin)
            .expect("robots policy should exist");

        if policy
            .disallow_rules
            .iter()
            .any(|rule| !rule.is_empty() && url.path().starts_with(rule))
        {
            return Err(AcquisitionError::BlockedByRobots(url.as_str().to_string()));
        }

        Ok(())
    }

    fn fetch_robots_policy(&self, url: &Url) -> Result<RobotsPolicy, AcquisitionError> {
        let Some(mut robots_url) = origin_key(url).and_then(|origin| Url::parse(&origin).ok())
        else {
            return Ok(RobotsPolicy::default());
        };
        robots_url.set_path("/robots.txt");
        robots_url.set_query(None);
        robots_url.set_fragment(None);

        let response = self.get(robots_url.as_str())?;
        if response.status().is_client_error() || response.status().is_server_error() {
            return Ok(RobotsPolicy::default());
        }

        let body = response.text().map_err(AcquisitionError::Http)?;
        Ok(parse_robots(body.as_str(), self.config.user_agent.as_str()))
    }
}

fn resolve_redirect(current_url: &Url, response: &Response) -> Result<Url, AcquisitionError> {
    let location = response
        .headers()
        .get(LOCATION)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| AcquisitionError::MissingRedirectLocation(current_url.to_string()))?;

    current_url.join(location).map_err(AcquisitionError::Url)
}

fn validate_content_type(content_type: &str) -> Result<(), AcquisitionError> {
    let normalized = content_type.to_ascii_lowercase();
    if normalized.starts_with("text/html") || normalized.starts_with("application/xhtml+xml") {
        Ok(())
    } else {
        Err(AcquisitionError::UnsupportedContentType(
            content_type.to_string(),
        ))
    }
}

fn normalize_cache_key(url: &str) -> Result<String, AcquisitionError> {
    if url.starts_with("fixture://") {
        return Ok(url.to_string());
    }

    let mut parsed = Url::parse(url).map_err(AcquisitionError::Url)?;
    parsed.set_fragment(None);

    Ok(parsed.to_string())
}

fn cache_key_hash(cache_key: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    cache_key.hash(&mut hasher);
    hasher.finish()
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}

fn parse_robots(body: &str, user_agent: &str) -> RobotsPolicy {
    let mut active = false;
    let mut matched_specific = false;
    let requested_agent = user_agent.to_ascii_lowercase();
    let mut disallow_rules = Vec::new();

    for raw_line in body.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        let Some((field, value)) = line.split_once(':') else {
            continue;
        };
        let field = field.trim().to_ascii_lowercase();
        let value = value.trim();

        if field == "user-agent" {
            let normalized = value.to_ascii_lowercase();
            active = normalized == "*" || requested_agent.contains(&normalized);
            if normalized != "*" && active {
                matched_specific = true;
                disallow_rules.clear();
            }
            continue;
        }

        if field == "disallow" && active && (matched_specific || !requested_agent.is_empty()) {
            disallow_rules.push(value.to_string());
        }
    }

    RobotsPolicy { disallow_rules }
}

fn origin_key(url: &Url) -> Option<String> {
    url.host_str().map(|_| {
        let port = url
            .port_or_known_default()
            .map(|value| format!(":{value}"))
            .unwrap_or_default();
        format!(
            "{}://{}{}",
            url.scheme(),
            url.host_str().unwrap_or_default(),
            port
        )
    })
}

#[derive(Debug, Error)]
pub enum AcquisitionError {
    #[error("unknown fixture source: {0}")]
    UnknownFixture(String),
    #[error("blocked by robots policy: {0}")]
    BlockedByRobots(String),
    #[error("unsupported content type: {0}")]
    UnsupportedContentType(String),
    #[error("too many redirects while fetching: {0}")]
    TooManyRedirects(String),
    #[error("missing redirect location for: {0}")]
    MissingRedirectLocation(String),
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("invalid url: {0}")]
    Url(#[from] url::ParseError),
    #[error("persistent cache I/O error: {0}")]
    PersistentCacheIo(std::io::Error),
    #[error("persistent cache JSON error: {0}")]
    PersistentCacheJson(serde_json::Error),
}

#[cfg(test)]
mod tests {
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

    use super::{AcquisitionConfig, AcquisitionEngine, AcquisitionError, FixtureResource};

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
                    let Ok(Some(request)) =
                        server.recv_timeout(std::time::Duration::from_millis(100))
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
}
