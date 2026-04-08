use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

use crate::{AcquiredDocument, AcquisitionConfig, AcquisitionError};
use touch_browser_contracts::CacheStatus;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct CachedDocument {
    pub(crate) result: AcquiredDocument,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistentCachedDocument {
    cache_key: String,
    fetched_at_unix_seconds: u64,
    result: AcquiredDocument,
}

pub(crate) fn load_persistent_cache(
    config: &AcquisitionConfig,
    cache_key: &str,
) -> Option<AcquiredDocument> {
    let cache_path = persistent_cache_path(config, cache_key)?;
    let raw = fs::read(&cache_path).ok()?;
    let cached: PersistentCachedDocument = serde_json::from_slice(&raw).ok()?;
    if cached.cache_key != cache_key {
        return None;
    }
    if cache_entry_expired(config, cached.fetched_at_unix_seconds) {
        let _ = fs::remove_file(cache_path);
        return None;
    }

    let mut result = cached.result;
    result.record.cache_status = CacheStatus::Hit;
    Some(result)
}

pub(crate) fn store_persistent_cache(
    config: &AcquisitionConfig,
    cache_key: &str,
    result: &AcquiredDocument,
) -> Result<(), AcquisitionError> {
    let Some(cache_path) = persistent_cache_path(config, cache_key) else {
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

pub(crate) fn persistent_cache_path(
    config: &AcquisitionConfig,
    cache_key: &str,
) -> Option<PathBuf> {
    let cache_dir = config.persistent_cache_dir.as_ref()?;
    Some(cache_dir.join(format!("{:016x}.json", stable_cache_key_hash(cache_key))))
}

pub(crate) fn cache_entry_expired(
    config: &AcquisitionConfig,
    fetched_at_unix_seconds: u64,
) -> bool {
    config.persistent_cache_ttl.is_zero()
        || current_unix_seconds().saturating_sub(fetched_at_unix_seconds)
            > config.persistent_cache_ttl.as_secs()
}

pub(crate) fn stable_cache_key_hash(cache_key: &str) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    let mut hash = OFFSET_BASIS;
    for byte in cache_key.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn current_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_secs()
}
