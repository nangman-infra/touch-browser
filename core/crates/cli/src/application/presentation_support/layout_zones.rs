use std::collections::BTreeSet;

use serde_json::Value;
use touch_browser_contracts::{SnapshotBlock, SnapshotBlockRole};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LayoutZone {
    Main,
    Nav,
    Aside,
    Header,
    Footer,
}

pub(super) fn block_layout_zone(block: &SnapshotBlock) -> Option<LayoutZone> {
    explicit_block_zone(block)
        .or_else(|| inferred_dom_path_zone(block))
        .or_else(|| inferred_role_zone(block))
}

fn explicit_block_zone(block: &SnapshotBlock) -> Option<LayoutZone> {
    match block.attributes.get("zone").and_then(Value::as_str) {
        Some("main") => Some(LayoutZone::Main),
        Some("nav") => Some(LayoutZone::Nav),
        Some("aside") => Some(LayoutZone::Aside),
        Some("header") => Some(LayoutZone::Header),
        Some("footer") => Some(LayoutZone::Footer),
        _ => None,
    }
}

fn inferred_role_zone(block: &SnapshotBlock) -> Option<LayoutZone> {
    match block.role {
        SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav => Some(LayoutZone::Nav),
        _ => None,
    }
}

fn inferred_dom_path_zone(block: &SnapshotBlock) -> Option<LayoutZone> {
    block
        .evidence
        .dom_path_hint
        .as_deref()?
        .split('>')
        .map(str::trim)
        .find_map(layout_zone_for_dom_segment)
}

fn layout_zone_for_dom_segment(segment: &str) -> Option<LayoutZone> {
    let normalized = segment.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }
    let marker_tokens = segment_marker_tokens(&normalized);

    if segment_matches_zone(&normalized, "footer")
        || segment_has_any_marker(
            &marker_tokens,
            &[
                "contentinfo",
                "site-footer",
                "page-footer",
                "article-footer",
                "article-footer--inner",
                "last-modified",
                "contributors",
            ],
        )
        || segment_has_marker_prefix(&marker_tokens, "footer-")
    {
        return Some(LayoutZone::Footer);
    }
    if segment_matches_zone(&normalized, "header")
        || segment_has_any_marker(
            &marker_tokens,
            &[
                "masthead",
                "banner",
                "mw-body-header",
                "vector-page-titlebar",
                "p-lang-btn",
                "mw-portlet-lang",
                "language-selector",
                "interlanguage-link",
            ],
        )
    {
        return Some(LayoutZone::Header);
    }
    if segment_matches_zone(&normalized, "aside")
        || segment_has_any_marker(&marker_tokens, &["sidebar", "side-panel", "mw-panel"])
    {
        return Some(LayoutZone::Aside);
    }
    if segment_matches_zone(&normalized, "nav")
        || segment_has_any_marker(
            &marker_tokens,
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
                "pagination",
                "tablist",
            ],
        )
    {
        return Some(LayoutZone::Nav);
    }
    if segment_matches_zone(&normalized, "main")
        || segment_matches_zone(&normalized, "article")
        || segment_has_any_marker(
            &marker_tokens,
            &[
                "main-content",
                "bodycontent",
                "mw-content-text",
                "mw-parser-output",
                "vector-body",
                "article-body",
                "content-area",
                "content-container",
                "content-body",
                "page-content",
                "docs-content",
                "doc-content",
                "docs-page",
                "reference-layout--body",
                "content-section",
                "mdx-content",
                "markdown",
                "article-content",
            ],
        )
        || segment_has_marker_prefix(&marker_tokens, "content-")
        || segment_has_marker_prefix(&marker_tokens, "docs-")
        || segment_has_marker_suffix(&marker_tokens, "prose")
    {
        return Some(LayoutZone::Main);
    }

    None
}

fn segment_matches_zone(segment: &str, zone: &str) -> bool {
    segment == zone
        || segment.starts_with(&format!("{zone}."))
        || segment.starts_with(&format!("{zone}#"))
        || segment.starts_with(&format!("{zone}["))
}

fn segment_marker_tokens(segment: &str) -> BTreeSet<String> {
    segment
        .split(['#', '.'])
        .map(str::trim)
        .filter(|token| !token.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn segment_has_any_marker(tokens: &BTreeSet<String>, markers: &[&str]) -> bool {
    markers.iter().any(|marker| tokens.contains(*marker))
}

fn segment_has_marker_prefix(tokens: &BTreeSet<String>, prefix: &str) -> bool {
    tokens.iter().any(|token| token.starts_with(prefix))
}

fn segment_has_marker_suffix(tokens: &BTreeSet<String>, suffix: &str) -> bool {
    tokens.iter().any(|token| token.ends_with(suffix))
}
