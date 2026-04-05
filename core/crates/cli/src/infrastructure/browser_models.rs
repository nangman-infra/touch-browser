use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::*;

#[derive(Debug, Clone)]
pub(crate) struct BrowserSessionContext {
    pub(crate) runtime: ReadOnlyRuntime,
    pub(crate) session: ReadOnlySession,
    pub(crate) snapshot: SnapshotDocument,
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

fn default_requested_budget() -> usize {
    DEFAULT_REQUESTED_TOKENS
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightSnapshotParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) budget: usize,
    pub(crate) headless: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) search_identity: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightSnapshotResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) mode: String,
    #[allow(dead_code)]
    pub(crate) source: String,
    pub(crate) final_url: String,
    #[allow(dead_code)]
    pub(crate) title: String,
    #[allow(dead_code)]
    pub(crate) visible_text: String,
    pub(crate) html: String,
    #[allow(dead_code)]
    pub(crate) html_length: usize,
    #[allow(dead_code)]
    pub(crate) link_count: usize,
    #[allow(dead_code)]
    pub(crate) button_count: usize,
    #[allow(dead_code)]
    pub(crate) input_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightFollowParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) target_ref: String,
    pub(crate) target_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_ordinal_hint: Option<usize>,
    pub(crate) headless: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightFollowResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) method: String,
    #[allow(dead_code)]
    pub(crate) limited_dynamic_action: bool,
    pub(crate) followed_ref: String,
    pub(crate) target_text: String,
    pub(crate) target_href: Option<String>,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightClickParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) target_ref: String,
    pub(crate) target_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_href: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_ordinal_hint: Option<usize>,
    pub(crate) headless: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightClickResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) method: String,
    #[allow(dead_code)]
    pub(crate) limited_dynamic_action: bool,
    pub(crate) clicked_ref: String,
    pub(crate) target_text: String,
    pub(crate) target_href: Option<String>,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightTypeParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) target_ref: String,
    pub(crate) target_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_ordinal_hint: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_input_type: Option<String>,
    pub(crate) value: String,
    pub(crate) headless: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightTypeResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) method: String,
    #[allow(dead_code)]
    pub(crate) limited_dynamic_action: bool,
    pub(crate) typed_ref: String,
    pub(crate) target_text: String,
    pub(crate) typed_length: usize,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightSubmitParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) target_ref: String,
    pub(crate) target_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_ordinal_hint: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) prefill: Vec<PlaywrightTypePrefill>,
    pub(crate) headless: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightSubmitResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) method: String,
    #[allow(dead_code)]
    pub(crate) limited_dynamic_action: bool,
    pub(crate) submitted_ref: String,
    pub(crate) target_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightTypePrefill {
    pub(crate) target_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_ordinal_hint: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_input_type: Option<String>,
    pub(crate) value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightPaginateParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) direction: String,
    pub(crate) current_page: usize,
    pub(crate) headless: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightPaginateResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) method: String,
    #[allow(dead_code)]
    pub(crate) limited_dynamic_action: bool,
    pub(crate) page: usize,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightExpandParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) html: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) context_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) profile_dir: Option<String>,
    pub(crate) target_ref: String,
    pub(crate) target_text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_tag_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) target_ordinal_hint: Option<usize>,
    pub(crate) headless: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightExpandResult {
    #[allow(dead_code)]
    pub(crate) status: String,
    #[allow(dead_code)]
    pub(crate) method: String,
    #[allow(dead_code)]
    pub(crate) limited_dynamic_action: bool,
    pub(crate) expanded_ref: String,
    pub(crate) target_text: String,
    pub(crate) clicked_text: String,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
}
