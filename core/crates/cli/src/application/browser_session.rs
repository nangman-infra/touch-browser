use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use touch_browser_contracts::{SearchReport, SnapshotDocument, SourceRisk};
use touch_browser_runtime::{ReadOnlyRuntime, ReadOnlySession};

use crate::{interface::cli_models::AckRisk, DEFAULT_REQUESTED_TOKENS};

#[derive(Debug, Clone)]
pub(crate) struct BrowserSessionContext {
    pub(crate) runtime: ReadOnlyRuntime,
    pub(crate) session: ReadOnlySession,
    pub(crate) snapshot: SnapshotDocument,
    pub(crate) source_risk: SourceRisk,
    pub(crate) source_label: Option<String>,
    pub(crate) browser_state: PersistedBrowserState,
    pub(crate) browser_context_dir: Option<String>,
    pub(crate) browser_profile_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserCliSession {
    pub(crate) version: String,
    pub(crate) headless: bool,
    #[serde(default = "default_requested_budget")]
    pub(crate) requested_budget: usize,
    pub(crate) session: ReadOnlySession,
    #[serde(default)]
    pub(crate) browser_state: Option<PersistedBrowserState>,
    #[serde(default)]
    pub(crate) browser_context_dir: Option<String>,
    #[serde(default)]
    pub(crate) browser_profile_dir: Option<String>,
    #[serde(default)]
    pub(crate) browser_origin: Option<BrowserOrigin>,
    #[serde(default)]
    pub(crate) allowlisted_domains: Vec<String>,
    #[serde(default)]
    pub(crate) browser_trace: Vec<BrowserActionTraceEntry>,
    #[serde(default)]
    pub(crate) approved_risks: BTreeSet<AckRisk>,
    #[serde(default)]
    pub(crate) latest_search: Option<SearchReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersistedBrowserState {
    pub(crate) current_url: String,
    pub(crate) current_html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserOrigin {
    pub(crate) target: String,
    pub(crate) source_risk: Option<SourceRisk>,
    pub(crate) source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct BrowserActionTraceEntry {
    pub(crate) action: String,
    pub(crate) timestamp: String,
    pub(crate) target_ref: Option<String>,
    pub(crate) direction: Option<String>,
    #[serde(default)]
    pub(crate) text_value: Option<String>,
    #[serde(default)]
    pub(crate) redacted: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct ObservedBrowserDocument {
    pub(crate) snapshot: SnapshotDocument,
    pub(crate) source_risk: SourceRisk,
    pub(crate) source_label: Option<String>,
    pub(crate) browser_state: PersistedBrowserState,
    pub(crate) browser_context_dir: Option<String>,
    pub(crate) browser_profile_dir: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct BrowserActionSource {
    pub(crate) source_url: String,
    pub(crate) url: Option<String>,
    pub(crate) html: Option<String>,
    pub(crate) context_dir: Option<String>,
    pub(crate) profile_dir: Option<String>,
    pub(crate) source_risk: SourceRisk,
    pub(crate) source_label: Option<String>,
}

fn default_requested_budget() -> usize {
    DEFAULT_REQUESTED_TOKENS
}
