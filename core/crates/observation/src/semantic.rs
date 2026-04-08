use std::collections::BTreeSet;

use kuchiki::{ElementData, NodeRef};
use touch_browser_contracts::{SnapshotBlockKind, SnapshotBlockRole};

pub(crate) fn semantic_tag(node: &NodeRef) -> Option<String> {
    let element = node.as_element()?;
    let role = element
        .attributes
        .borrow()
        .get("role")
        .map(|value| value.to_ascii_lowercase());
    match role.as_deref() {
        Some("combobox") => Some("combobox".to_string()),
        Some("listbox") => Some("listbox".to_string()),
        _ => {
            let tag = element.name.local.to_string();
            is_semantic_tag(&tag).then_some(tag)
        }
    }
}

pub(crate) fn is_nested_duplicate(node: &NodeRef, tag: &str) -> bool {
    match tag {
        "a" => has_ancestor_tag(node, &["a"]),
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => has_ancestor_tag(node, &["table"]),
        "ul" | "ol" => has_ancestor_tag(node, &["li"]),
        _ => false,
    }
}

pub(crate) fn semantic_zone(node: &NodeRef, tag: &str) -> &'static str {
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
    } else if has_ancestor_marker(
        node,
        &[
            "article-footer",
            "article-footer__inner",
            "article-footer--inner",
            "last-modified",
            "contributors",
        ],
    ) {
        "footer"
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
                "reference-layout__body",
                "reference-layout--body",
                "content-section",
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

pub(crate) fn semantic_kind(tag: &str) -> SnapshotBlockKind {
    match tag {
        "title" => SnapshotBlockKind::Metadata,
        "script" => SnapshotBlockKind::Metadata,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => SnapshotBlockKind::Heading,
        "a" => SnapshotBlockKind::Link,
        "listbox" => SnapshotBlockKind::List,
        "ul" | "ol" => SnapshotBlockKind::List,
        "table" => SnapshotBlockKind::Table,
        "form" => SnapshotBlockKind::Form,
        "button" => SnapshotBlockKind::Button,
        "input" | "select" | "combobox" => SnapshotBlockKind::Input,
        _ => SnapshotBlockKind::Text,
    }
}

pub(crate) fn semantic_role(node: &NodeRef, tag: &str, zone: &str) -> SnapshotBlockRole {
    if tag == "title" {
        SnapshotBlockRole::Metadata
    } else if tag == "script" {
        SnapshotBlockRole::Supporting
    } else if zone == "nav" {
        SnapshotBlockRole::PrimaryNav
    } else if zone == "aside" {
        SnapshotBlockRole::SecondaryNav
    } else if matches!(tag, "form" | "button" | "input" | "select" | "combobox") {
        SnapshotBlockRole::FormControl
    } else if tag == "listbox" {
        SnapshotBlockRole::Supporting
    } else if tag == "a" && link_is_external(node) {
        SnapshotBlockRole::Cta
    } else {
        SnapshotBlockRole::Content
    }
}

pub(crate) fn candidate_priority(kind: &SnapshotBlockKind, role: &SnapshotBlockRole) -> usize {
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

pub(crate) fn kind_slug(kind: &SnapshotBlockKind) -> &'static str {
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
            | "select"
            | "script"
    )
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
