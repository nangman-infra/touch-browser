mod budget;
mod hidden_rules;
mod semantic;
mod text;

use std::collections::{BTreeMap, HashMap};

use budget::apply_budget;
use hidden_rules::{is_hidden, HiddenRules};
use kuchiki::{parse_html, traits::TendrilSink, NodeRef};
use semantic::{
    candidate_priority, is_nested_duplicate, kind_slug, semantic_kind, semantic_role, semantic_tag,
    semantic_zone,
};
use serde_json::{json, Value};
use text::{
    candidate_slug, dom_path_hint, estimate_tokens, extract_semantic_text, extract_title,
    hostile_signal_hint, semantic_attributes,
};
use thiserror::Error;
use touch_browser_contracts::{
    SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument,
    SnapshotEvidence, SnapshotSource, SourceType, CONTRACT_VERSION, STABLE_REF_VERSION,
};

pub fn crate_status() -> &'static str {
    "observation ready"
}

pub fn recommend_requested_tokens(html: &str, requested_tokens: usize) -> usize {
    if requested_tokens != 512 {
        return requested_tokens.max(1);
    }

    let html_len = html.len();
    let link_count = html.matches("<a").count();
    let heading_count = (1..=6)
        .map(|level| html.matches(&format!("<h{level}")).count())
        .sum::<usize>();
    let paragraph_count = html.matches("<p").count();
    let list_item_count = html.matches("<li").count();
    let table_count = html.matches("<table").count();
    let button_count = html.matches("<button").count();
    let input_count = html.matches("<input").count();

    let complexity_score = (html_len / 12_000)
        + (link_count / 20)
        + (heading_count / 10)
        + (paragraph_count / 30)
        + (list_item_count / 40)
        + (table_count * 3)
        + (button_count / 10)
        + (input_count / 10);

    if html_len >= 150_000 || link_count >= 120 || complexity_score >= 24 {
        4096
    } else if html_len >= 60_000 || link_count >= 45 || complexity_score >= 10 {
        2048
    } else if html_len >= 20_000 || link_count >= 20 || complexity_score >= 4 {
        1024
    } else {
        512
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationInput {
    pub source_url: String,
    pub source_type: SourceType,
    pub html: String,
    pub requested_tokens: usize,
}

impl ObservationInput {
    pub fn new(
        source_url: impl Into<String>,
        source_type: SourceType,
        html: impl Into<String>,
        requested_tokens: usize,
    ) -> Self {
        Self {
            source_url: source_url.into(),
            source_type,
            html: html.into(),
            requested_tokens,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ObservationCompiler;

impl ObservationCompiler {
    pub fn compile(&self, input: &ObservationInput) -> Result<SnapshotDocument, ObservationError> {
        if input.requested_tokens == 0 {
            return Err(ObservationError::ZeroBudget);
        }

        let document = parse_html().one(input.html.clone());
        let hidden_rules = HiddenRules::from_document(&document);
        let title = extract_title(&document);
        let mut candidates = collect_candidates(&document, input, &hidden_rules)?;

        if candidates.is_empty() {
            return Err(ObservationError::NoSemanticBlocks);
        }

        let estimated_tokens = candidates
            .iter()
            .map(|candidate| candidate.token_cost)
            .sum();
        let selected = apply_budget(input.requested_tokens, &mut candidates);
        let emitted_tokens = selected.iter().map(|candidate| candidate.token_cost).sum();
        let truncated = emitted_tokens < estimated_tokens;
        let blocks = selected
            .into_iter()
            .enumerate()
            .map(|(index, candidate)| candidate.into_snapshot_block(index + 1, input))
            .collect::<Vec<_>>();

        Ok(SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: input.source_url.clone(),
                source_type: input.source_type.clone(),
                title,
            },
            budget: SnapshotBudget {
                requested_tokens: input.requested_tokens,
                estimated_tokens,
                emitted_tokens,
                truncated,
            },
            blocks,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ObservationError {
    #[error("observation input budget must be at least 1")]
    ZeroBudget,
    #[error("observation compiler found no semantic blocks")]
    NoSemanticBlocks,
    #[error("selector query failed: {0}")]
    InvalidSelection(String),
}

#[derive(Debug, Clone)]
pub(crate) struct CandidateBlock {
    order: usize,
    priority: usize,
    token_cost: usize,
    zone: &'static str,
    kind: SnapshotBlockKind,
    role: SnapshotBlockRole,
    stable_ref: String,
    text: String,
    attributes: BTreeMap<String, Value>,
    dom_path_hint: String,
}

impl CandidateBlock {
    fn into_snapshot_block(self, index: usize, input: &ObservationInput) -> SnapshotBlock {
        SnapshotBlock {
            version: CONTRACT_VERSION.to_string(),
            id: format!("b{index}"),
            kind: self.kind,
            stable_ref: self.stable_ref,
            role: self.role,
            text: self.text,
            attributes: self.attributes,
            evidence: SnapshotEvidence {
                source_url: input.source_url.clone(),
                source_type: input.source_type.clone(),
                dom_path_hint: Some(self.dom_path_hint),
                byte_range_start: None,
                byte_range_end: None,
            },
        }
    }
}

fn collect_candidates(
    document: &NodeRef,
    input: &ObservationInput,
    hidden_rules: &HiddenRules,
) -> Result<Vec<CandidateBlock>, ObservationError> {
    let mut ref_counts: HashMap<String, usize> = HashMap::new();
    let mut candidates = Vec::new();
    let mut order = 0usize;

    for node in document.descendants() {
        if node.as_element().is_none() {
            continue;
        }

        let Some(tag) = semantic_tag(&node) else {
            continue;
        };

        if is_hidden(&node, hidden_rules) || is_nested_duplicate(&node, &tag) {
            continue;
        }

        let text = extract_semantic_text(&node, &tag, hidden_rules)?;
        if text.is_empty() {
            continue;
        }

        let zone = semantic_zone(&node, &tag);
        let kind = semantic_kind(&tag);
        let role = semantic_role(&node, &tag, zone);
        let slug = candidate_slug(&node, &tag, &text);
        let base_ref = format!("r{zone}:{}:{slug}", kind_slug(&kind));
        let count = ref_counts.entry(base_ref.clone()).or_insert(0);
        *count += 1;

        let stable_ref = if *count == 1 {
            base_ref
        } else {
            format!("{base_ref}:{}", *count)
        };

        let mut attributes = semantic_attributes(&node, &tag, &text, hidden_rules)?;
        attributes.insert("zone".to_string(), json!(zone));
        attributes.insert("tagName".to_string(), json!(tag));
        if let Some(ancestor_signal) = hostile_signal_hint(input.source_url.as_str(), &text) {
            attributes.insert("hostileHint".to_string(), json!(ancestor_signal));
        }

        let priority = candidate_priority(&kind, &role);
        let token_cost = estimate_tokens(&text);
        let dom_path_hint = dom_path_hint(&node);

        candidates.push(CandidateBlock {
            order,
            priority,
            token_cost,
            zone,
            kind,
            role,
            stable_ref,
            text,
            attributes,
            dom_path_hint,
        });
        order += 1;
    }

    candidates.sort_by_key(|candidate| candidate.order);
    Ok(candidates)
}

#[cfg(test)]
mod tests;
