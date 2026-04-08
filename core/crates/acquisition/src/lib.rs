use std::{
    collections::{BTreeMap, HashMap},
    path::PathBuf,
    time::Duration,
};

mod cache;
mod robots;

use cache::{load_persistent_cache, store_persistent_cache, CachedDocument};
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_TYPE, LOCATION, USER_AGENT};
use robots::{
    fetch_robots_policy, load_persistent_robots_policy, origin_key, store_persistent_robots_policy,
    RobotsPolicy,
};
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
    pub request_timeout: Duration,
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
            request_timeout: Duration::from_secs(10),
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
            .timeout(config.request_timeout)
            .connect_timeout(config.request_timeout)
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
            if let Some(cached) = load_persistent_cache(&self.config, &cache_key) {
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
            let _ = store_persistent_cache(&self.config, &cache_key, &result);
            let _ = store_persistent_cache(&self.config, &result.record.final_url.clone(), &result);
        }

        Ok(result)
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
            let policy =
                if let Some(cached_policy) = load_persistent_robots_policy(&self.config, &origin) {
                    cached_policy
                } else {
                    fetch_robots_policy(&self.config, url, |next_url| self.get(next_url))?
                };
            let _ = store_persistent_robots_policy(&self.config, &origin, &policy);
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
mod tests;
