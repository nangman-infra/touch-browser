use std::collections::BTreeMap;

use kuchiki::NodeRef;
use serde_json::{json, Value};

use crate::{
    hidden_rules::{has_hidden_ancestor_within, HiddenRules},
    ObservationError,
};

pub(crate) fn extract_title(document: &NodeRef) -> Option<String> {
    document
        .select_first("title")
        .ok()
        .map(|node| normalize_text(&node.text_contents()))
        .filter(|text| !text.is_empty())
}

pub(crate) fn extract_semantic_text(
    node: &NodeRef,
    tag: &str,
    hidden_rules: &HiddenRules,
) -> Result<String, ObservationError> {
    match tag {
        "title" => Ok(normalize_text(&node.text_contents())),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "p" | "a" | "button" | "form" => {
            Ok(extract_visible_text(node, hidden_rules))
        }
        "ul" | "ol" => extract_list_text(node, tag == "ol", hidden_rules),
        "table" => extract_table_text(node, hidden_rules),
        "input" => Ok(extract_input_label(node)),
        "select" => extract_select_text(node, hidden_rules),
        "combobox" => extract_combobox_text(node, hidden_rules),
        "listbox" => extract_listbox_text(node, hidden_rules),
        "script" => Ok(extract_script_semantic_text(node)),
        _ => Ok(String::new()),
    }
}

pub(crate) fn semantic_attributes(
    node: &NodeRef,
    tag: &str,
    text: &str,
    hidden_rules: &HiddenRules,
) -> Result<BTreeMap<String, Value>, ObservationError> {
    let mut attributes = BTreeMap::new();

    match tag {
        "title" => insert_source_attribute(&mut attributes, "title"),
        "script" => insert_script_attributes(node, &mut attributes),
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => insert_heading_attributes(tag, &mut attributes),
        "a" => insert_link_attributes(node, &mut attributes),
        "ul" | "ol" => insert_list_attributes(node, tag, &mut attributes)?,
        "table" => insert_table_attributes(node, &mut attributes)?,
        "form" => insert_form_attributes(node, &mut attributes)?,
        "input" => insert_input_attributes(node, &mut attributes),
        "select" => insert_select_attributes(node, hidden_rules, &mut attributes)?,
        "combobox" => insert_combobox_attributes(node, hidden_rules, &mut attributes),
        "listbox" => insert_listbox_attributes(node, hidden_rules, &mut attributes),
        _ => {}
    }

    attributes.insert("textLength".to_string(), json!(text.chars().count()));
    Ok(attributes)
}

pub(crate) fn candidate_slug(node: &NodeRef, tag: &str, text: &str) -> String {
    if let Some(slug) = element_slug(node, tag) {
        return slug;
    }

    fallback_slug(tag, text)
}

pub(crate) fn estimate_tokens(text: &str) -> usize {
    let approx = text.chars().count().div_ceil(4);
    approx.max(1)
}

pub(crate) fn dom_path_hint(node: &NodeRef) -> String {
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

pub(crate) fn hostile_signal_hint(source_url: &str, text: &str) -> Option<&'static str> {
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

pub(crate) fn normalize_text(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_visible_text(node: &NodeRef, hidden_rules: &HiddenRules) -> String {
    let text = node
        .descendants()
        .filter_map(|descendant| {
            let text_node = descendant.as_text()?;
            if has_hidden_ancestor_within(&descendant, node, hidden_rules) {
                return None;
            }
            let normalized = normalize_text(&text_node.borrow());
            (!normalized.is_empty()).then_some(normalized)
        })
        .collect::<Vec<_>>()
        .join(" ");
    normalize_text(&text)
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

fn extract_list_text(
    node: &NodeRef,
    ordered: bool,
    hidden_rules: &HiddenRules,
) -> Result<String, ObservationError> {
    let items = node
        .select("li")
        .map_err(|_| ObservationError::InvalidSelection("li".to_string()))?
        .enumerate()
        .filter_map(|(index, item)| {
            let item_text = extract_visible_text(item.as_node(), hidden_rules);
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

fn extract_table_text(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
) -> Result<String, ObservationError> {
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
                let cell_text = extract_visible_text(cell.as_node(), hidden_rules);
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

fn extract_select_text(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
) -> Result<String, ObservationError> {
    let current = selected_select_options(node, hidden_rules)?;
    let options = select_option_labels(node, hidden_rules)?;
    let mut parts = control_descriptor_parts(node);
    if !current.is_empty() {
        parts.push(format!("current {}", current.join(" | ")));
    }
    if !options.is_empty() {
        parts.push(format!("options {}", options.join(" | ")));
    }

    Ok(normalize_text(&parts.join(" ")))
}

fn extract_combobox_text(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
) -> Result<String, ObservationError> {
    let mut parts = control_descriptor_parts(node);
    let current = extract_visible_text(node, hidden_rules);
    if !current.is_empty() {
        parts.push(format!("current {current}"));
    }

    if let Some(listbox) = controlled_popup(node, "listbox") {
        let options = listbox_option_labels(&listbox, hidden_rules);
        if !options.is_empty() {
            parts.push(format!("options {}", options.join(" | ")));
        }
    }

    Ok(normalize_text(&parts.join(" ")))
}

fn extract_listbox_text(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
) -> Result<String, ObservationError> {
    let mut parts = control_descriptor_parts(node);
    let options = listbox_option_labels(node, hidden_rules);
    if !options.is_empty() {
        parts.push(format!("options {}", options.join(" | ")));
    } else {
        let fallback = extract_visible_text(node, hidden_rules);
        if !fallback.is_empty() {
            parts.push(fallback);
        }
    }

    Ok(normalize_text(&parts.join(" ")))
}

fn control_descriptor_parts(node: &NodeRef) -> Vec<String> {
    let Some(element) = node.as_element() else {
        return Vec::new();
    };
    let attrs = element.attributes.borrow();
    let mut parts = Vec::new();
    for key in ["name", "aria-label", "title", "placeholder"] {
        if let Some(value) = attrs.get(key) {
            let normalized = normalize_text(value);
            if !normalized.is_empty() {
                parts.push(normalized);
            }
        }
    }
    parts
}

fn selected_select_options(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
) -> Result<Vec<String>, ObservationError> {
    let mut selected = node
        .select("option[selected]")
        .map_err(|_| ObservationError::InvalidSelection("option[selected]".to_string()))?
        .filter_map(|option| {
            let text = extract_visible_text(option.as_node(), hidden_rules);
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>();
    if !selected.is_empty() {
        return Ok(selected);
    }

    selected = node
        .select("option")
        .map_err(|_| ObservationError::InvalidSelection("option".to_string()))?
        .find_map(|option| {
            let text = extract_visible_text(option.as_node(), hidden_rules);
            (!text.is_empty()).then_some(vec![text])
        })
        .unwrap_or_default();

    Ok(selected)
}

fn select_option_labels(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
) -> Result<Vec<String>, ObservationError> {
    Ok(node
        .select("option")
        .map_err(|_| ObservationError::InvalidSelection("option".to_string()))?
        .filter_map(|option| {
            let text = extract_visible_text(option.as_node(), hidden_rules);
            (!text.is_empty()).then_some(text)
        })
        .collect())
}

fn controlled_popup(node: &NodeRef, expected_role: &str) -> Option<NodeRef> {
    let element = node.as_element()?;
    let attrs = element.attributes.borrow();
    let popup_id = attrs.get("aria-controls")?;
    let root = node.ancestors().last()?;

    root.descendants().find(|candidate| {
        let Some(candidate_element) = candidate.as_element() else {
            return false;
        };
        let candidate_attrs = candidate_element.attributes.borrow();
        candidate_attrs.get("id") == Some(popup_id)
            && candidate_attrs
                .get("role")
                .map(|role| role.eq_ignore_ascii_case(expected_role))
                .unwrap_or(false)
    })
}

fn listbox_option_labels(node: &NodeRef, hidden_rules: &HiddenRules) -> Vec<String> {
    let mut options = node
        .descendants()
        .filter_map(|candidate| {
            let element = candidate.as_element()?;
            let attrs = element.attributes.borrow();
            let role = attrs.get("role")?;
            if !role.eq_ignore_ascii_case("option") {
                return None;
            }
            let text = extract_visible_text(&candidate, hidden_rules);
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>();

    if !options.is_empty() {
        options.sort();
        options.dedup();
        return options;
    }

    options = node
        .select("li")
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let text = extract_visible_text(item.as_node(), hidden_rules);
            (!text.is_empty()).then_some(text)
        })
        .collect::<Vec<_>>();
    options.sort();
    options.dedup();
    options
}

fn insert_source_attribute(attributes: &mut BTreeMap<String, Value>, source: &str) {
    attributes.insert("source".to_string(), json!(source));
}

fn insert_script_attributes(node: &NodeRef, attributes: &mut BTreeMap<String, Value>) {
    if let Some(element) = node.as_element() {
        let attrs = element.attributes.borrow();
        if let Some(script_type) = attrs.get("type") {
            attributes.insert("scriptType".to_string(), json!(script_type));
        }
        if let Some(script_id) = attrs.get("id") {
            attributes.insert("scriptId".to_string(), json!(script_id));
        }
    }
    insert_source_attribute(attributes, "script");
}

fn insert_heading_attributes(tag: &str, attributes: &mut BTreeMap<String, Value>) {
    let level = tag.trim_start_matches('h').parse::<usize>().unwrap_or(1);
    attributes.insert("level".to_string(), json!(level));
}

fn insert_link_attributes(node: &NodeRef, attributes: &mut BTreeMap<String, Value>) {
    if let Some(element) = node.as_element() {
        let attrs = element.attributes.borrow();
        if let Some(href) = attrs.get("href") {
            attributes.insert("href".to_string(), json!(href));
            attributes.insert("external".to_string(), json!(is_external_href(href)));
        }
    }
}

fn insert_list_attributes(
    node: &NodeRef,
    tag: &str,
    attributes: &mut BTreeMap<String, Value>,
) -> Result<(), ObservationError> {
    let items = node
        .select("li")
        .map_err(|_| ObservationError::InvalidSelection("li".to_string()))?
        .count();
    attributes.insert("ordered".to_string(), json!(tag == "ol"));
    attributes.insert("items".to_string(), json!(items));
    Ok(())
}

fn insert_table_attributes(
    node: &NodeRef,
    attributes: &mut BTreeMap<String, Value>,
) -> Result<(), ObservationError> {
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
    Ok(())
}

fn insert_form_attributes(
    node: &NodeRef,
    attributes: &mut BTreeMap<String, Value>,
) -> Result<(), ObservationError> {
    let controls = node
        .select("input, select, textarea, button")
        .map_err(|_| {
            ObservationError::InvalidSelection("input, select, textarea, button".to_string())
        })?
        .count();
    attributes.insert("controls".to_string(), json!(controls));
    Ok(())
}

fn insert_input_attributes(node: &NodeRef, attributes: &mut BTreeMap<String, Value>) {
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

fn insert_select_attributes(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
    attributes: &mut BTreeMap<String, Value>,
) -> Result<(), ObservationError> {
    let options = select_option_labels(node, hidden_rules)?;
    let selected = selected_select_options(node, hidden_rules)?;
    attributes.insert("optionCount".to_string(), json!(options.len()));
    if !options.is_empty() {
        attributes.insert("options".to_string(), json!(options));
        attributes.insert("selectionSemantic".to_string(), json!("available-options"));
    }
    if !selected.is_empty() {
        attributes.insert("selectedOptions".to_string(), json!(selected));
    }
    Ok(())
}

fn insert_combobox_attributes(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
    attributes: &mut BTreeMap<String, Value>,
) {
    if let Some(element) = node.as_element() {
        let attrs = element.attributes.borrow();
        if let Some(expanded) = attrs.get("aria-expanded") {
            attributes.insert("expanded".to_string(), json!(expanded == "true"));
        }
        if let Some(controls) = attrs.get("aria-controls") {
            attributes.insert("controls".to_string(), json!(controls));
        }
    }

    let current_value = extract_visible_text(node, hidden_rules);
    if !current_value.is_empty() {
        attributes.insert("currentValue".to_string(), json!(current_value));
    }
    if let Some(listbox) = controlled_popup(node, "listbox") {
        let options = listbox_option_labels(&listbox, hidden_rules);
        if !options.is_empty() {
            attributes.insert("options".to_string(), json!(options));
            attributes.insert("selectionSemantic".to_string(), json!("available-options"));
        }
    }
}

fn insert_listbox_attributes(
    node: &NodeRef,
    hidden_rules: &HiddenRules,
    attributes: &mut BTreeMap<String, Value>,
) {
    let options = listbox_option_labels(node, hidden_rules);
    attributes.insert("optionCount".to_string(), json!(options.len()));
    if !options.is_empty() {
        attributes.insert("options".to_string(), json!(options));
        attributes.insert("selectionSemantic".to_string(), json!("available-options"));
    }
}

fn element_slug(node: &NodeRef, tag: &str) -> Option<String> {
    let element = node.as_element()?;
    let attrs = element.attributes.borrow();

    slug_from_value(attrs.get("id")).or_else(|| {
        if tag == "a" {
            slug_from_value(attrs.get("href"))
        } else {
            None
        }
    })
}

fn slug_from_value(value: Option<&str>) -> Option<String> {
    let slug = slugify(value?);
    if slug == "block" {
        None
    } else {
        Some(slug)
    }
}

fn fallback_slug(tag: &str, text: &str) -> String {
    let slug = slugify(text);
    if slug == "block" {
        tag.to_string()
    } else {
        slug
    }
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

fn is_external_href(href: &str) -> bool {
    href.starts_with("http://") || href.starts_with("https://")
}
