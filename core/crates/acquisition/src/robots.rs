use std::fs;

use reqwest::blocking::Response;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{
    cache::{cache_entry_expired, stable_cache_key_hash},
    AcquisitionConfig, AcquisitionError,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RobotsPolicy {
    pub(crate) disallow_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentRobotsPolicy {
    origin: String,
    fetched_at_unix_seconds: u64,
    policy: RobotsPolicy,
}

pub(crate) fn load_persistent_robots_policy(
    config: &AcquisitionConfig,
    origin: &str,
) -> Option<RobotsPolicy> {
    let cache_path = robots_cache_path(config, origin)?;
    let raw = fs::read(&cache_path).ok()?;
    let cached: PersistentRobotsPolicy = serde_json::from_slice(&raw).ok()?;
    if cached.origin != origin {
        return None;
    }
    if cache_entry_expired(config, cached.fetched_at_unix_seconds) {
        let _ = fs::remove_file(cache_path);
        return None;
    }

    Some(cached.policy)
}

pub(crate) fn store_persistent_robots_policy(
    config: &AcquisitionConfig,
    origin: &str,
    policy: &RobotsPolicy,
) -> Result<(), AcquisitionError> {
    let Some(cache_path) = robots_cache_path(config, origin) else {
        return Ok(());
    };
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(AcquisitionError::PersistentCacheIo)?;
    }

    let cached = PersistentRobotsPolicy {
        origin: origin.to_string(),
        fetched_at_unix_seconds: current_unix_seconds(),
        policy: policy.clone(),
    };
    fs::write(
        cache_path,
        serde_json::to_vec(&cached).map_err(AcquisitionError::PersistentCacheJson)?,
    )
    .map_err(AcquisitionError::PersistentCacheIo)?;
    Ok(())
}

pub(crate) fn fetch_robots_policy<F>(
    config: &AcquisitionConfig,
    url: &Url,
    mut get: F,
) -> Result<RobotsPolicy, AcquisitionError>
where
    F: FnMut(&str) -> Result<Response, AcquisitionError>,
{
    let Some(mut robots_url) = origin_key(url).and_then(|origin| Url::parse(&origin).ok()) else {
        return Ok(RobotsPolicy::default());
    };
    robots_url.set_path("/robots.txt");
    robots_url.set_query(None);
    robots_url.set_fragment(None);

    let response = match get(robots_url.as_str()) {
        Ok(response) => response,
        Err(_) => return Ok(RobotsPolicy::default()),
    };
    if response.status().is_client_error() || response.status().is_server_error() {
        return Ok(RobotsPolicy::default());
    }

    let body = match response.text() {
        Ok(body) => body,
        Err(_) => return Ok(RobotsPolicy::default()),
    };
    Ok(parse_robots(body.as_str(), config.user_agent.as_str()))
}

pub(crate) fn parse_robots(body: &str, user_agent: &str) -> RobotsPolicy {
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

pub(crate) fn origin_key(url: &Url) -> Option<String> {
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

fn robots_cache_path(config: &AcquisitionConfig, origin: &str) -> Option<std::path::PathBuf> {
    let cache_dir = config.persistent_cache_dir.as_ref()?;
    Some(
        cache_dir
            .join("robots")
            .join(format!("{:016x}.json", stable_cache_key_hash(origin))),
    )
}

fn current_unix_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs()
}
