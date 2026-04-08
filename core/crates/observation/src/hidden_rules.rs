use std::collections::BTreeSet;

use kuchiki::{ElementData, NodeRef};
use regex::Regex;

#[derive(Debug, Default)]
pub(crate) struct HiddenRules {
    hidden_classes: BTreeSet<String>,
}

impl HiddenRules {
    pub(crate) fn from_document(document: &NodeRef) -> Self {
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

pub(crate) fn is_hidden(node: &NodeRef, hidden_rules: &HiddenRules) -> bool {
    node.ancestors().any(|ancestor| {
        let Some(element) = ancestor.as_element() else {
            return false;
        };
        element_hides_node(element, hidden_rules)
    })
}

pub(crate) fn has_hidden_ancestor_within(
    node: &NodeRef,
    root: &NodeRef,
    hidden_rules: &HiddenRules,
) -> bool {
    for ancestor in node.ancestors() {
        if let Some(element) = ancestor.as_element() {
            if element_hides_node(element, hidden_rules) {
                return true;
            }
        }
        if ancestor == *root {
            return false;
        }
    }

    false
}

fn element_hides_node(element: &ElementData, hidden_rules: &HiddenRules) -> bool {
    hidden_tag(element.name.local.as_ref())
        || hidden_by_attributes(element)
        || hidden_by_style(element)
        || hidden_by_class(element, hidden_rules)
}

fn hidden_tag(tag: &str) -> bool {
    matches!(tag, "style" | "noscript" | "template")
}

fn hidden_by_attributes(element: &ElementData) -> bool {
    let attrs = element.attributes.borrow();
    attrs.contains("hidden") || attrs.get("aria-hidden") == Some("true")
}

fn hidden_by_style(element: &ElementData) -> bool {
    let attrs = element.attributes.borrow();
    let Some(style) = attrs.get("style") else {
        return false;
    };
    let normalized = style.to_ascii_lowercase();
    normalized.contains("display:none")
        || normalized.contains("display: none")
        || normalized.contains("visibility:hidden")
        || normalized.contains("visibility: hidden")
}

fn hidden_by_class(element: &ElementData, hidden_rules: &HiddenRules) -> bool {
    let attrs = element.attributes.borrow();
    let Some(class_attr) = attrs.get("class") else {
        return false;
    };

    class_attr
        .split_whitespace()
        .any(|class_name| hidden_rules.hidden_classes.contains(class_name))
}
