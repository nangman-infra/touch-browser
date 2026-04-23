use serde::{Deserialize, Serialize};

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
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub(crate) manual_recovery: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PlaywrightLoadDiagnostics {
    pub(crate) wait_strategy: String,
    pub(crate) wait_budget_ms: Option<usize>,
    pub(crate) wait_consumed_ms: Option<usize>,
    pub(crate) wait_stop_reason: Option<String>,
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
    pub(crate) diagnostics: PlaywrightLoadDiagnostics,
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
    pub(crate) diagnostics: PlaywrightLoadDiagnostics,
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
pub(crate) struct PlaywrightDownloadEvidence {
    pub(crate) completed: bool,
    pub(crate) suggested_filename: String,
    pub(crate) path: Option<String>,
    pub(crate) byte_length: Option<u64>,
    pub(crate) sha256: Option<String>,
    pub(crate) failure: Option<String>,
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
    pub(crate) download: Option<PlaywrightDownloadEvidence>,
    pub(crate) final_url: String,
    pub(crate) title: String,
    pub(crate) visible_text: String,
    pub(crate) html: String,
    pub(crate) diagnostics: PlaywrightLoadDiagnostics,
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
    pub(crate) diagnostics: PlaywrightLoadDiagnostics,
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
