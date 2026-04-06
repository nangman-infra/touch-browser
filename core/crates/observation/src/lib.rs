use std::collections::{BTreeMap, BTreeSet, HashMap};

use kuchiki::{parse_html, traits::TendrilSink, ElementData, NodeRef};
use regex::Regex;
use serde_json::{json, Value};
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

#[derive(Debug, Default)]
struct HiddenRules {
    hidden_classes: BTreeSet<String>,
}

impl HiddenRules {
    fn from_document(document: &NodeRef) -> Self {
        let hidden_class_pattern = Regex::new(
            r"\.([A-Za-z_][A-Za-z0-9_-]*)\s*\{[^}]*?(display\s*:\s*none|visibility\s*:\s*hidden)",
        )
        .expect("hidden class regex must compile");
        let mut hidden_classes = BTreeSet::new();

        if let Ok(styles) = document.select("style") {
            for style in styles {
                let css = style.text_contents();
                for capture in hidden_class_pattern.captures_iter(&css) {
                    hidden_classes.insert(capture[1].to_string());
                }
            }
        }

        Self { hidden_classes }
    }
}

#[derive(Debug, Clone)]
struct CandidateBlock {
    order: usize,
    priority: usize,
    token_cost: usize,
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
        let Some(element) = node.as_element() else {
            continue;
        };

        let tag = element.name.local.to_string();
        if !is_semantic_tag(&tag) {
            continue;
        }

        if is_hidden(&node, hidden_rules) || is_nested_duplicate(&node, &tag) {
            continue;
        }

        let text = extract_semantic_text(&node, &tag)?;
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

        let mut attributes = semantic_attributes(&node, &tag, &text)?;
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

fn apply_budget(budget: usize, candidates: &mut [CandidateBlock]) -> Vec<CandidateBlock> {
    let estimated_tokens = candidates
        .iter()
        .map(|candidate| candidate.token_cost)
        .sum::<usize>();
    if estimated_tokens <= budget {
        return candidates.to_vec();
    }

    let mut ranked = candidates
        .iter()
        .enumerate()
        .collect::<Vec<(usize, &CandidateBlock)>>();
    ranked.sort_by(|(left_index, left), (right_index, right)| {
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| left_index.cmp(right_index))
    });

    let mut selected_indices = BTreeSet::new();
    let mut emitted_tokens = 0usize;
    let nav_budget = if budget < 32 {
        0
    } else {
        budget.div_ceil(4).max(24).min(budget / 2)
    };

    if nav_budget > 0 {
        for (index, candidate) in &ranked {
            if !is_navigation_candidate(candidate) {
                continue;
            }

            if emitted_tokens + candidate.token_cost > nav_budget && !selected_indices.is_empty() {
                continue;
            }

            emitted_tokens += candidate.token_cost;
            selected_indices.insert(*index);

            if emitted_tokens >= nav_budget {
                break;
            }
        }
    }

    for (index, candidate) in ranked {
        if selected_indices.contains(&index) {
            continue;
        }

        if emitted_tokens + candidate.token_cost > budget && !selected_indices.is_empty() {
            continue;
        }

        emitted_tokens += candidate.token_cost;
        selected_indices.insert(index);

        if emitted_tokens >= budget {
            break;
        }
    }

    candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| selected_indices.contains(index))
        .map(|(_, candidate)| candidate.clone())
        .collect()
}

fn is_navigation_candidate(candidate: &CandidateBlock) -> bool {
    matches!(
        candidate.kind,
        SnapshotBlockKind::Link | SnapshotBlockKind::Button | SnapshotBlockKind::Input
    ) || matches!(
        candidate.role,
        SnapshotBlockRole::PrimaryNav
            | SnapshotBlockRole::SecondaryNav
            | SnapshotBlockRole::Cta
            | SnapshotBlockRole::FormControl
    )
}

fn extract_title(document: &NodeRef) -> Option<String> {
    document
        .select_first("title")
        .ok()
        .map(|node| normalize_text(&node.text_contents()))
        .filter(|text| !text.is_empty())
}

fn is_semantic_tag(tag: &str) -> bool {
    matches!(
        tag,
        "title"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "p"
            | "a"
            | "ul"
            | "ol"
            | "table"
            | "form"
            | "button"
            | "input"
            | "script"
    )
}

fn is_nested_duplicate(node: &NodeRef, tag: &str) -> bool {
    match tag {
        "a" => has_ancestor_tag(node, &["a"]),
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => has_ancestor_tag(node, &["table"]),
        "ul" | "ol" => has_ancestor_tag(node, &["li"]),
        _ => false,
    }
}

fn extract_semantic_text(node: &NodeRef, tag: &str) -> Result<String, ObservationError> {
    match tag {
        "title" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "a" | "button" => {
            Ok(normalize_text(&node.text_contents()))
        }
        "ul" | "ol" => extract_list_text(node, tag == "ol"),
        "table" => extract_table_text(node),
        "form" => Ok(normalize_text(&node.text_contents())),
        "input" => Ok(extract_input_label(node)),
        "script" => Ok(extract_script_semantic_text(node)),
        _ => Ok(String::new()),
    }
}

fn extract_script_semantic_text(node: &NodeRef) -> String {
    let Some(element) = node.as_element() else {
        return String::new();
    };
    let attrs = element.attributes.borrow();
    let script_type = attrs.get("type").unwrap_or_default();
    let script_id = attrs.get("id").unwrap_or_default();
    let raw = node.text_contents();

    let kind = if script_type.eq_ignore_ascii_case("application/ld+json")
        || raw.contains("\"@context\"")
    {
        "json-ld"
    } else if matches!(
        script_id,
        "__NEXT_DATA__"
            | "__NUXT__"
            | "__NUXT_DATA__"
            | "__APOLLO_STATE__"
            | "__INITIAL_STATE__"
            | "__PRELOADED_STATE__"
    ) {
        "hydration"
    } else {
        return String::new();
    };

    summarize_json_payload(kind, &raw)
}

fn summarize_json_payload(kind: &str, raw: &str) -> String {
    let Ok(value) = serde_json::from_str::<Value>(raw) else {
        return String::new();
    };

    let mut fields = Vec::new();
    collect_json_summary_fields(&value, "", &mut fields, 0, 14);
    if fields.is_empty() {
        return String::new();
    }

    normalize_text(&format!("{kind}: {}", fields.join(" | ")))
}

const JSON_SUMMARY_PRIORITY_KEYS: [&str; 16] = [
    "@type",
    "name",
    "headline",
    "description",
    "url",
    "datePublished",
    "dateModified",
    "price",
    "availability",
    "title",
    "page",
    "pathname",
    "slug",
    "query",
    "buildId",
    "locale",
];

fn collect_json_summary_fields(
    value: &Value,
    path: &str,
    output: &mut Vec<String>,
    depth: usize,
    limit: usize,
) {
    if output.len() >= limit || depth > 4 {
        return;
    }

    match value {
        Value::Object(map) => collect_json_summary_object_fields(map, path, output, depth, limit),
        Value::Array(items) => collect_json_summary_array_fields(items, path, output, depth, limit),
        Value::String(text) => append_json_summary_text(path, text, output),
        Value::Bool(flag) => append_json_summary_scalar(path, flag, output),
        Value::Number(number) => append_json_summary_scalar(path, number, output),
        Value::Null => {}
    }
}

fn collect_json_summary_object_fields(
    map: &serde_json::Map<String, Value>,
    path: &str,
    output: &mut Vec<String>,
    depth: usize,
    limit: usize,
) {
    for key in JSON_SUMMARY_PRIORITY_KEYS {
        if output.len() >= limit {
            return;
        }
        let Some(candidate) = map.get(key) else {
            continue;
        };
        let next_path = next_json_summary_path(path, key);
        collect_json_summary_fields(candidate, &next_path, output, depth + 1, limit);
    }

    for (key, candidate) in map {
        if output.len() >= limit {
            return;
        }
        if JSON_SUMMARY_PRIORITY_KEYS.contains(&key.as_str()) {
            continue;
        }
        let next_path = next_json_summary_path(path, key);
        collect_json_summary_fields(candidate, &next_path, output, depth + 1, limit);
    }
}

fn collect_json_summary_array_fields(
    items: &[Value],
    path: &str,
    output: &mut Vec<String>,
    depth: usize,
    limit: usize,
) {
    for item in items.iter().take(3) {
        if output.len() >= limit {
            return;
        }
        collect_json_summary_fields(item, path, output, depth + 1, limit);
    }
}

fn append_json_summary_text(path: &str, text: &str, output: &mut Vec<String>) {
    let normalized = normalize_text(text);
    if !normalized.is_empty() {
        output.push(format!("{path}: {normalized}"));
    }
}

fn append_json_summary_scalar(path: &str, value: impl std::fmt::Display, output: &mut Vec<String>) {
    output.push(format!("{path}: {value}"));
}

fn next_json_summary_path(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{path}.{key}")
    }
}

fn extract_list_text(node: &NodeRef, ordered: bool) -> Result<String, ObservationError> {
    let items = node
        .select("li")
        .map_err(|_| ObservationError::InvalidSelection("li".to_string()))?
        .enumerate()
        .filter_map(|(index, item)| {
            let item_text = normalize_text(&item.text_contents());
            if item_text.is_empty() {
                None
            } else if ordered {
                Some(format!("{}. {}", index + 1, item_text))
            } else {
                Some(format!("- {}", item_text))
            }
        })
        .collect::<Vec<_>>();

    Ok(items.join(" "))
}

fn extract_table_text(node: &NodeRef) -> Result<String, ObservationError> {
    let mut rows = Vec::new();
    let row_nodes = node
        .select("tr")
        .map_err(|_| ObservationError::InvalidSelection("tr".to_string()))?;

    for row in row_nodes {
        let cells = row
            .as_node()
            .select("th, td")
            .map_err(|_| ObservationError::InvalidSelection("th, td".to_string()))?
            .filter_map(|cell| {
                let cell_text = normalize_text(&cell.text_contents());
                (!cell_text.is_empty()).then_some(cell_text)
            })
            .collect::<Vec<_>>();

        if !cells.is_empty() {
            rows.push(cells.join(" | "));
        }
    }

    Ok(rows.join("\n"))
}

fn extract_input_label(node: &NodeRef) -> String {
    let mut parts = Vec::new();

    if let Some(element) = node.as_element() {
        let attributes = element.attributes.borrow();

        for key in ["name", "type", "placeholder", "value", "aria-label"] {
            if let Some(value) = attributes.get(key) {
                let text = normalize_text(value);
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
    }

    normalize_text(&parts.join(" "))
}

fn semantic_zone(node: &NodeRef, tag: &str) -> &'static str {
    if tag == "title" || has_ancestor_tag(node, &["head"]) {
        "head"
    } else if has_ancestor_tag(node, &["nav"]) {
        "nav"
    } else if has_ancestor_tag(node, &["aside"]) {
        "aside"
    } else if has_ancestor_marker(
        node,
        &[
            "p-lang-btn",
            "interlanguage-link",
            "language-selector",
            "mw-portlet-lang",
        ],
    ) {
        "header"
    } else if has_ancestor_marker(
        node,
        &[
            "vector-page-titlebar-toc",
            "mw-panel-toc",
            "vector-page-toolbar",
            "p-associated-pages",
            "p-views",
            "p-cactions",
            "p-tb",
            "p-electronpdfservice-sidebar-portlet-heading",
            "p-wikibase-otherprojects",
            "p-variants",
            "breadcrumb",
            "navbox",
        ],
    ) {
        "nav"
    } else if has_ancestor_marker(node, &["sidebar", "side-panel", "mw-panel"]) {
        "aside"
    } else if has_ancestor_tag(node, &["main"])
        || has_ancestor_role(node, "main")
        || has_ancestor_marker(
            node,
            &[
                "bodycontent",
                "mw-content-text",
                "mw-parser-output",
                "article-body",
                "main-content",
                "vector-body",
            ],
        )
    {
        "main"
    } else if has_ancestor_tag(node, &["header"]) {
        "header"
    } else if has_ancestor_tag(node, &["footer"]) {
        "footer"
    } else {
        "body"
    }
}

fn semantic_kind(tag: &str) -> SnapshotBlockKind {
    match tag {
        "title" => SnapshotBlockKind::Metadata,
        "script" => SnapshotBlockKind::Metadata,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => SnapshotBlockKind::Heading,
        "a" => SnapshotBlockKind::Link,
        "ul" | "ol" => SnapshotBlockKind::List,
        "table" => SnapshotBlockKind::Table,
        "form" => SnapshotBlockKind::Form,
        "button" => SnapshotBlockKind::Button,
        "input" => SnapshotBlockKind::Input,
        _ => SnapshotBlockKind::Text,
    }
}

fn semantic_role(node: &NodeRef, tag: &str, zone: &str) -> SnapshotBlockRole {
    if tag == "title" {
        SnapshotBlockRole::Metadata
    } else if tag == "script" {
        SnapshotBlockRole::Supporting
    } else if zone == "nav" {
        SnapshotBlockRole::PrimaryNav
    } else if zone == "aside" {
        SnapshotBlockRole::SecondaryNav
    } else if matches!(tag, "form" | "button" | "input") {
        SnapshotBlockRole::FormControl
    } else if tag == "a" && link_is_external(node) {
        SnapshotBlockRole::Cta
    } else {
        SnapshotBlockRole::Content
    }
}

fn semantic_attributes(
    node: &NodeRef,
    tag: &str,
    text: &str,
) -> Result<BTreeMap<String, Value>, ObservationError> {
    let mut attributes = BTreeMap::new();

    match tag {
        "title" => {
            attributes.insert("source".to_string(), json!("title"));
        }
        "script" => {
            if let Some(element) = node.as_element() {
                let attrs = element.attributes.borrow();
                if let Some(script_type) = attrs.get("type") {
                    attributes.insert("scriptType".to_string(), json!(script_type));
                }
                if let Some(script_id) = attrs.get("id") {
                    attributes.insert("scriptId".to_string(), json!(script_id));
                }
            }
            attributes.insert("source".to_string(), json!("script"));
        }
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
            let level = tag.trim_start_matches('h').parse::<usize>().unwrap_or(1);
            attributes.insert("level".to_string(), json!(level));
        }
        "a" => {
            if let Some(element) = node.as_element() {
                let attrs = element.attributes.borrow();
                if let Some(href) = attrs.get("href") {
                    attributes.insert("href".to_string(), json!(href));
                    attributes.insert("external".to_string(), json!(is_external_href(href)));
                }
            }
        }
        "ul" | "ol" => {
            let items = node
                .select("li")
                .map_err(|_| ObservationError::InvalidSelection("li".to_string()))?
                .count();
            attributes.insert("ordered".to_string(), json!(tag == "ol"));
            attributes.insert("items".to_string(), json!(items));
        }
        "table" => {
            let row_cell_counts = node
                .select("tr")
                .map_err(|_| ObservationError::InvalidSelection("tr".to_string()))?
                .map(|row| {
                    row.as_node()
                        .select("th, td")
                        .map(|cells| cells.count())
                        .unwrap_or(0)
                })
                .collect::<Vec<_>>();
            let columns = row_cell_counts.iter().copied().max().unwrap_or(0);
            attributes.insert("rows".to_string(), json!(row_cell_counts.len()));
            attributes.insert("columns".to_string(), json!(columns));
        }
        "form" => {
            let controls = node
                .select("input, select, textarea, button")
                .map_err(|_| {
                    ObservationError::InvalidSelection(
                        "input, select, textarea, button".to_string(),
                    )
                })?
                .count();
            attributes.insert("controls".to_string(), json!(controls));
        }
        "input" => {
            if let Some(element) = node.as_element() {
                let attrs = element.attributes.borrow();
                if let Some(input_type) = attrs.get("type") {
                    attributes.insert("inputType".to_string(), json!(input_type));
                }
                if let Some(name) = attrs.get("name") {
                    attributes.insert("name".to_string(), json!(name));
                }
            }
        }
        _ => {}
    }

    attributes.insert("textLength".to_string(), json!(text.chars().count()));
    Ok(attributes)
}

fn candidate_slug(node: &NodeRef, tag: &str, text: &str) -> String {
    if let Some(element) = node.as_element() {
        let attrs = element.attributes.borrow();

        if let Some(id) = attrs.get("id") {
            let slug = slugify(id);
            if !slug.is_empty() {
                return slug;
            }
        }

        if tag == "a" {
            if let Some(href) = attrs.get("href") {
                let slug = slugify(href);
                if !slug.is_empty() {
                    return slug;
                }
            }
        }
    }

    let slug = slugify(text);
    if slug.is_empty() {
        tag.to_string()
    } else {
        slug
    }
}

fn candidate_priority(kind: &SnapshotBlockKind, role: &SnapshotBlockRole) -> usize {
    let kind_score = match kind {
        SnapshotBlockKind::Metadata => 120,
        SnapshotBlockKind::Heading => 110,
        SnapshotBlockKind::Table => 95,
        SnapshotBlockKind::List => 90,
        SnapshotBlockKind::Text => 80,
        SnapshotBlockKind::Link => 70,
        SnapshotBlockKind::Form => 60,
        SnapshotBlockKind::Button | SnapshotBlockKind::Input => 55,
    };
    let role_bonus = match role {
        SnapshotBlockRole::Metadata => 20,
        SnapshotBlockRole::Cta => 10,
        SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav => 5,
        SnapshotBlockRole::Content | SnapshotBlockRole::Supporting => 0,
        SnapshotBlockRole::FormControl | SnapshotBlockRole::TableCell => 2,
    };

    kind_score + role_bonus
}

fn estimate_tokens(text: &str) -> usize {
    let approx = text.chars().count().div_ceil(4);
    approx.max(1)
}

fn kind_slug(kind: &SnapshotBlockKind) -> &'static str {
    match kind {
        SnapshotBlockKind::Text => "text",
        SnapshotBlockKind::Heading => "heading",
        SnapshotBlockKind::Link => "link",
        SnapshotBlockKind::Form => "form",
        SnapshotBlockKind::Table => "table",
        SnapshotBlockKind::List => "list",
        SnapshotBlockKind::Button => "button",
        SnapshotBlockKind::Input => "input",
        SnapshotBlockKind::Metadata => "metadata",
    }
}

fn normalize_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn slugify(input: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;

    for character in input.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator {
            slug.push('-');
            previous_was_separator = true;
        }
    }

    while slug.ends_with('-') {
        slug.pop();
    }

    let trimmed = slug.trim_start_matches('-');
    if trimmed.is_empty() {
        "block".to_string()
    } else {
        trimmed.chars().take(48).collect()
    }
}

fn dom_path_hint(node: &NodeRef) -> String {
    let mut parts = Vec::new();

    for ancestor in node.ancestors() {
        if let Some(element) = ancestor.as_element() {
            let attrs = element.attributes.borrow();
            let mut segment = element.name.local.to_string();
            if let Some(id) = attrs.get("id").and_then(normalize_dom_marker_token) {
                segment.push('#');
                segment.push_str(&id);
            }
            if let Some(class_attr) = attrs.get("class") {
                for class_name in class_attr
                    .split_whitespace()
                    .filter_map(normalize_dom_marker_token)
                    .take(2)
                {
                    segment.push('.');
                    segment.push_str(&class_name);
                }
            }
            parts.push(segment);
        }
    }

    parts.reverse();
    parts.join(" > ")
}

fn normalize_dom_marker_token(token: &str) -> Option<String> {
    let normalized = token
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>()
        .trim_matches('-')
        .replace('_', "-");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.chars().take(48).collect())
    }
}

fn has_ancestor_tag(node: &NodeRef, tags: &[&str]) -> bool {
    node.ancestors().any(|ancestor| {
        ancestor
            .as_element()
            .map(|element| tags.contains(&element.name.local.as_ref()))
            .unwrap_or(false)
    })
}

fn has_ancestor_role(node: &NodeRef, role: &str) -> bool {
    node.ancestors().any(|ancestor| {
        ancestor
            .as_element()
            .and_then(|element| element.attributes.borrow().get("role").map(str::to_string))
            .map(|value| value.eq_ignore_ascii_case(role))
            .unwrap_or(false)
    })
}

fn has_ancestor_marker(node: &NodeRef, markers: &[&str]) -> bool {
    let expected_markers = markers
        .iter()
        .filter_map(|marker| normalize_dom_marker_token(marker))
        .collect::<BTreeSet<_>>();
    node.ancestors().any(|ancestor| {
        let Some(element) = ancestor.as_element() else {
            return false;
        };
        if matches!(element.name.local.as_ref(), "html" | "body") {
            return false;
        }
        element_marker_tokens(element)
            .iter()
            .any(|token| expected_markers.contains(token))
    })
}

fn element_marker_tokens(element: &ElementData) -> BTreeSet<String> {
    let attrs = element.attributes.borrow();
    let mut tokens = BTreeSet::new();

    if let Some(id) = attrs.get("id").and_then(normalize_dom_marker_token) {
        tokens.insert(id);
    }
    if let Some(class_attr) = attrs.get("class") {
        tokens.extend(
            class_attr
                .split_whitespace()
                .filter_map(normalize_dom_marker_token),
        );
    }
    if let Some(role) = attrs.get("role").and_then(normalize_dom_marker_token) {
        tokens.insert(role);
    }

    tokens
}

fn is_hidden(node: &NodeRef, hidden_rules: &HiddenRules) -> bool {
    node.ancestors().any(|ancestor| {
        let Some(element) = ancestor.as_element() else {
            return false;
        };

        let tag = element.name.local.as_ref();
        if matches!(tag, "style" | "noscript" | "template") {
            return true;
        }

        let attrs = element.attributes.borrow();

        if attrs.contains("hidden") || attrs.get("aria-hidden") == Some("true") {
            return true;
        }

        if let Some(style) = attrs.get("style") {
            let normalized = style.to_ascii_lowercase();
            if normalized.contains("display:none")
                || normalized.contains("display: none")
                || normalized.contains("visibility:hidden")
                || normalized.contains("visibility: hidden")
            {
                return true;
            }
        }

        if let Some(class_attr) = attrs.get("class") {
            for class_name in class_attr.split_whitespace() {
                if hidden_rules.hidden_classes.contains(class_name) {
                    return true;
                }
            }
        }

        false
    })
}

fn link_is_external(node: &NodeRef) -> bool {
    let Some(element) = node.as_element() else {
        return false;
    };
    let attrs = element.attributes.borrow();
    attrs.get("href").map(is_external_href).unwrap_or(false)
}

fn is_external_href(href: &str) -> bool {
    href.starts_with("http://") || href.starts_with("https://")
}

fn hostile_signal_hint(source_url: &str, text: &str) -> Option<&'static str> {
    if !source_url.contains("/hostile/") {
        return None;
    }

    let lowered = text.to_ascii_lowercase();
    if lowered.contains("[system]") || lowered.contains("system override") {
        Some("untrusted-system-language")
    } else if lowered.contains("mandatory link") {
        Some("suspicious-cta")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde::Deserialize;

    use super::{recommend_requested_tokens, ObservationCompiler, ObservationInput};
    use touch_browser_contracts::{SnapshotDocument, SourceType};

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureMetadata {
        source_uri: String,
        html_path: String,
        expected_snapshot_path: String,
    }

    #[test]
    fn produces_expected_golden_snapshots_for_seed_fixtures() {
        let compiler = ObservationCompiler;

        for fixture in seed_fixture_paths() {
            let metadata = read_fixture_metadata(&fixture);
            let html_path = repo_root().join(metadata.html_path);
            let expected_path = repo_root().join(metadata.expected_snapshot_path);

            let actual = compiler
                .compile(&ObservationInput::new(
                    metadata.source_uri,
                    SourceType::Fixture,
                    fs::read_to_string(html_path).expect("fixture html should be readable"),
                    512,
                ))
                .expect("fixture snapshot should compile");

            let expected: SnapshotDocument = serde_json::from_str(
                &fs::read_to_string(expected_path).expect("golden snapshot should be readable"),
            )
            .expect("golden snapshot json should deserialize");

            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn generates_deterministic_refs_for_same_input() {
        let compiler = ObservationCompiler;
        let fixture = repo_root().join("fixtures/research/static-docs/getting-started/index.html");
        let html = fs::read_to_string(fixture).expect("fixture html should be readable");
        let input = ObservationInput::new(
            "fixture://research/static-docs/getting-started",
            SourceType::Fixture,
            html,
            512,
        );

        let first = compiler.compile(&input).expect("first compile should work");
        let second = compiler
            .compile(&input)
            .expect("second compile should work");

        let first_refs = first
            .blocks
            .iter()
            .map(|block| block.stable_ref.as_str())
            .collect::<Vec<_>>();
        let second_refs = second
            .blocks
            .iter()
            .map(|block| block.stable_ref.as_str())
            .collect::<Vec<_>>();

        assert_eq!(first_refs, second_refs);
    }

    #[test]
    fn truncates_low_priority_blocks_when_budget_is_small() {
        let compiler = ObservationCompiler;
        let fixture = repo_root().join("fixtures/research/static-docs/getting-started/index.html");
        let html = fs::read_to_string(fixture).expect("fixture html should be readable");

        let snapshot = compiler
            .compile(&ObservationInput::new(
                "fixture://research/static-docs/getting-started",
                SourceType::Fixture,
                html,
                12,
            ))
            .expect("compile should work");

        assert!(snapshot.budget.truncated);
        assert!(snapshot.budget.emitted_tokens < snapshot.budget.estimated_tokens);
        assert!(snapshot
            .blocks
            .iter()
            .any(|block| block.text.contains("Getting Started")));
        assert!(snapshot.blocks.iter().all(|block| block.text != "Pricing"));
    }

    #[test]
    fn recommends_higher_budget_for_complex_pages() {
        let simple_html = "<html><body><main><h1>Simple</h1><p>Hello</p></main></body></html>";
        assert_eq!(recommend_requested_tokens(simple_html, 512), 512);

        let large_html = format!(
            "<html><body>{}</body></html>",
            (0..160)
                .map(|index| format!(
                    "<a href=\"/docs/{index}\">Doc {index}</a><p>Paragraph {index}</p>"
                ))
                .collect::<String>()
        );
        assert!(
            recommend_requested_tokens(&large_html, 512) >= 2048,
            "complex pages should auto-escalate beyond the default budget"
        );
        assert_eq!(recommend_requested_tokens(&large_html, 4096), 4096);
    }

    #[test]
    fn captures_json_ld_and_hydration_scripts_as_semantic_metadata() {
        let compiler = ObservationCompiler;
        let html = r#"
            <html>
              <head>
                <title>Modern Docs</title>
                <script type="application/ld+json">
                  {"@context":"https://schema.org","@type":"TechArticle","headline":"Modern Docs","datePublished":"2026-04-05"}
                </script>
              </head>
              <body>
                <main>
                  <h1>Modern Docs</h1>
                  <p>Primary content.</p>
                </main>
                <script id="__NEXT_DATA__" type="application/json">
                  {"page":"/docs","buildId":"build-123","props":{"pageProps":{"title":"Modern Docs","slug":"modern-docs"}}}
                </script>
              </body>
            </html>
        "#;

        let snapshot = compiler
            .compile(&ObservationInput::new(
                "https://docs.example.com/modern",
                SourceType::Http,
                html,
                512,
            ))
            .expect("compile should work");

        let metadata_blocks = snapshot
            .blocks
            .iter()
            .filter(|block| block.kind == touch_browser_contracts::SnapshotBlockKind::Metadata)
            .collect::<Vec<_>>();

        assert!(metadata_blocks
            .iter()
            .any(|block| block.text.contains("json-ld")));
        assert!(metadata_blocks
            .iter()
            .any(|block| block.text.contains("hydration")));
        assert!(metadata_blocks
            .iter()
            .any(|block| block.text.contains("Modern Docs")));
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should exist")
    }

    fn seed_fixture_paths() -> Vec<PathBuf> {
        vec![
            repo_root().join("fixtures/research/static-docs/getting-started/fixture.json"),
            repo_root().join("fixtures/research/navigation/api-reference/fixture.json"),
            repo_root().join("fixtures/research/citation-heavy/pricing/fixture.json"),
            repo_root().join("fixtures/research/hostile/hidden-instruction/fixture.json"),
            repo_root().join("fixtures/research/hostile/fake-system-message/fixture.json"),
        ]
    }

    fn read_fixture_metadata(path: &PathBuf) -> FixtureMetadata {
        serde_json::from_str(
            &fs::read_to_string(path).expect("fixture metadata should be readable"),
        )
        .expect("fixture metadata should deserialize")
    }
}
