mod block_filters;
mod compact_render;
mod layout_zones;
mod markdown_render;

use std::collections::BTreeSet;

use self::{
    block_filters::{
        keep_app_main_read_view_block, keep_app_main_reading_block, keep_compact_block,
        keep_main_read_view_block, keep_main_reading_block, keep_navigation_block,
        keep_read_view_block,
    },
    compact_render::render_compact_block,
    layout_zones::{block_layout_zone, LayoutZone},
    markdown_render::render_markdown_block,
};
use touch_browser_contracts::{
    CompactRefIndexEntry, SessionSynthesisClaim, SessionSynthesisReport, SnapshotBlockKind,
    SnapshotDocument,
};

#[cfg(test)]
use touch_browser_contracts::{SnapshotBlock, SnapshotBlockRole};

pub(crate) fn render_compact_snapshot(snapshot: &SnapshotDocument) -> String {
    let has_heading = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, SnapshotBlockKind::Heading));

    snapshot
        .blocks
        .iter()
        .filter(|block| keep_compact_block(block, has_heading))
        .map(render_compact_block)
        .collect::<Vec<_>>()
        .join("\n")
}

fn is_interactive_app_main_surface(snapshot: &SnapshotDocument) -> bool {
    let mut heading_count = 0usize;
    let mut substantial_text_count = 0usize;
    let mut substantial_list_count = 0usize;
    let mut interactive_count = 0usize;

    for block in &snapshot.blocks {
        if block_layout_zone(block) != Some(LayoutZone::Main) {
            continue;
        }

        let char_count = block.text.trim().chars().count();
        match block.kind {
            SnapshotBlockKind::Heading => {
                if char_count >= 4 {
                    heading_count += 1;
                }
            }
            SnapshotBlockKind::Text => {
                if char_count >= 40 {
                    substantial_text_count += 1;
                }
            }
            SnapshotBlockKind::List | SnapshotBlockKind::Table => {
                if char_count >= 80 {
                    substantial_list_count += 1;
                }
            }
            SnapshotBlockKind::Link
            | SnapshotBlockKind::Button
            | SnapshotBlockKind::Input
            | SnapshotBlockKind::Form => interactive_count += 1,
            SnapshotBlockKind::Metadata => {}
        }
    }

    (interactive_count >= 8
        && heading_count <= 2
        && substantial_text_count == 0
        && substantial_list_count == 0)
        || (interactive_count >= 4
            && heading_count <= 1
            && substantial_text_count == 0
            && substantial_list_count == 0)
        || (interactive_count >= 3
            && heading_count == 0
            && substantial_text_count == 0
            && substantial_list_count == 0)
}

fn app_title_fallback_markdown(snapshot: &SnapshotDocument) -> Option<String> {
    snapshot
        .source
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!("# {title}"))
}

fn app_title_fallback_compact(snapshot: &SnapshotDocument) -> Option<String> {
    snapshot
        .source
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(|title| format!("h1 {title}"))
}

pub(crate) fn render_reading_compact_snapshot(snapshot: &SnapshotDocument) -> String {
    let has_heading = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, SnapshotBlockKind::Heading));
    let has_main_zone = snapshot
        .blocks
        .iter()
        .any(|block| block_layout_zone(block) == Some(LayoutZone::Main));
    let app_like_main_surface = has_main_zone && is_interactive_app_main_surface(snapshot);

    let rendered = snapshot
        .blocks
        .iter()
        .filter(|block| {
            if app_like_main_surface {
                keep_app_main_reading_block(block, has_heading, has_main_zone)
            } else {
                keep_main_reading_block(block, has_heading, has_main_zone)
            }
        })
        .map(render_compact_block)
        .collect::<Vec<_>>()
        .join("\n");

    if rendered.is_empty() && app_like_main_surface {
        return app_title_fallback_compact(snapshot).unwrap_or(rendered);
    }

    rendered
}

pub(crate) fn render_read_view_markdown(snapshot: &SnapshotDocument) -> String {
    let has_heading = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, SnapshotBlockKind::Heading));

    snapshot
        .blocks
        .iter()
        .filter(|block| keep_read_view_block(block, has_heading))
        .map(render_markdown_block)
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub(crate) fn render_main_read_view_markdown(snapshot: &SnapshotDocument) -> String {
    let has_heading = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, SnapshotBlockKind::Heading));
    let has_main_zone = snapshot
        .blocks
        .iter()
        .any(|block| block_layout_zone(block) == Some(LayoutZone::Main));
    let app_like_main_surface = has_main_zone && is_interactive_app_main_surface(snapshot);

    let rendered = snapshot
        .blocks
        .iter()
        .filter(|block| {
            if app_like_main_surface {
                keep_app_main_read_view_block(block, has_heading, has_main_zone)
            } else {
                keep_main_read_view_block(block, has_heading, has_main_zone)
            }
        })
        .map(render_markdown_block)
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n");

    if rendered.is_empty() && app_like_main_surface {
        return app_title_fallback_markdown(snapshot).unwrap_or(rendered);
    }

    rendered
}

pub(crate) fn render_navigation_compact_snapshot(snapshot: &SnapshotDocument) -> String {
    snapshot
        .blocks
        .iter()
        .filter(|block| keep_navigation_block(block))
        .map(render_compact_block)
        .collect::<Vec<_>>()
        .join("\n")
}

pub(crate) fn compact_ref_index(snapshot: &SnapshotDocument) -> Vec<CompactRefIndexEntry> {
    snapshot
        .blocks
        .iter()
        .map(|block| CompactRefIndexEntry {
            id: block.id.clone(),
            stable_ref: block.stable_ref.clone(),
            kind: block.kind.clone(),
        })
        .collect()
}

pub(crate) fn navigation_ref_index(snapshot: &SnapshotDocument) -> Vec<CompactRefIndexEntry> {
    snapshot
        .blocks
        .iter()
        .filter(|block| keep_navigation_block(block))
        .map(|block| CompactRefIndexEntry {
            id: block.id.clone(),
            stable_ref: block.stable_ref.clone(),
            kind: block.kind.clone(),
        })
        .collect()
}

pub(crate) fn render_session_synthesis_markdown(report: &SessionSynthesisReport) -> String {
    let mut sections = vec![
        "# Session Synthesis".to_string(),
        String::new(),
        format!("- Session ID: {}", report.session_id),
        format!("- Snapshots: {}", report.snapshot_count),
        format!("- Evidence Reports: {}", report.evidence_report_count),
    ];

    if !report.visited_urls.is_empty() {
        sections.push(format!(
            "- Visited URLs: {}",
            report.visited_urls.join(", ")
        ));
    }

    if !report.synthesized_notes.is_empty() {
        sections.push(String::new());
        sections.push("## Synthesized Notes".to_string());
        for note in &report.synthesized_notes {
            sections.push(format!("- {note}"));
        }
    }

    if !report.supported_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Evidence-Supported Claims".to_string());
        for claim in &report.supported_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    if !report.contradicted_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Contradicted Claims".to_string());
        for claim in &report.contradicted_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    if !report.unsupported_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Insufficient Evidence Claims".to_string());
        for claim in &report.unsupported_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    if !report.needs_more_browsing_claims.is_empty() {
        sections.push(String::new());
        sections.push("## Needs More Browsing Claims".to_string());
        for claim in &report.needs_more_browsing_claims {
            sections.push(render_session_claim_markdown(claim));
        }
    }

    sections.join("\n")
}

fn render_session_claim_markdown(claim: &SessionSynthesisClaim) -> String {
    let mut lines = vec![format!("- {}", claim.statement)];

    if !claim.citations.is_empty() {
        let citations = claim
            .citations
            .iter()
            .map(|citation| citation.url.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        lines.push(format!("  Citations: {}", citations.join(", ")));
    }

    if !claim.support_refs.is_empty() {
        lines.push(format!("  Refs: {}", claim.support_refs.join(", ")));
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::{
        compact_ref_index, render_compact_snapshot, render_main_read_view_markdown,
        render_read_view_markdown, render_reading_compact_snapshot,
        render_session_synthesis_markdown, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole,
        SnapshotDocument,
    };
    use serde_json::json;
    use std::collections::BTreeMap;
    use touch_browser_contracts::{
        SessionSynthesisClaim, SessionSynthesisClaimStatus, SessionSynthesisReport, SnapshotBudget,
        SnapshotEvidence, SnapshotSource, SourceRisk, SourceType, CONTRACT_VERSION,
        STABLE_REF_VERSION,
    };

    #[test]
    fn renders_compact_snapshot_lines() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "fixture://research/navigation/browser-follow".to_string(),
                source_type: SourceType::Fixture,
                title: Some("Browser Follow".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 24,
                emitted_tokens: 24,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:browser-follow".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Browser Follow".to_string(),
                    attributes: BTreeMap::from([("level".to_string(), json!(1))]),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:docs".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Open docs".to_string(),
                    attributes: BTreeMap::from([("href".to_string(), json!("#docs"))]),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        assert_eq!(
            render_compact_snapshot(&snapshot),
            "h1 Browser Follow\na Open docs"
        );
        assert_eq!(
            compact_ref_index(&snapshot)
                .into_iter()
                .map(|entry| (entry.id, entry.stable_ref))
                .collect::<Vec<_>>(),
            vec![
                ("b1".to_string(), "rmain:heading:browser-follow".to_string()),
                ("b2".to_string(), "rmain:link:docs".to_string()),
            ]
        );
    }

    #[test]
    fn renders_read_view_markdown_with_full_text() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "fixture://research/navigation/browser-follow".to_string(),
                source_type: SourceType::Fixture,
                title: Some("Browser Follow".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 24,
                emitted_tokens: 24,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:browser-follow".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Browser Follow".to_string(),
                    attributes: BTreeMap::from([("level".to_string(), json!(1))]),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:details".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "This page explains how the browser-backed runtime keeps evidence links intact across steps.".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:docs".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Open docs".to_string(),
                    attributes: BTreeMap::from([(
                        "href".to_string(),
                        json!("https://example.com/docs"),
                    )]),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        assert_eq!(
            render_read_view_markdown(&snapshot),
            "# Browser Follow\n\nThis page explains how the browser-backed runtime keeps evidence links intact across steps.\n\n- [Open docs](https://example.com/docs)"
        );
    }

    #[test]
    fn renders_main_read_view_without_navigation_noise() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://www.iana.org/help/example-domains".to_string(),
                source_type: SourceType::Http,
                title: Some("Example Domains".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rnav:link:domains".to_string(),
                    role: SnapshotBlockRole::PrimaryNav,
                    text: "Domains".to_string(),
                    attributes: BTreeMap::from([
                        ("href".to_string(), json!("/domains")),
                        ("zone".to_string(), json!("nav")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.iana.org/help/example-domains".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > header > nav > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:example-domains".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Example Domains".to_string(),
                    attributes: BTreeMap::from([
                        ("level".to_string(), json!(1)),
                        ("zone".to_string(), json!("main")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.iana.org/help/example-domains".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:summary".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "As described in RFC 2606 and RFC 6761, example domains are reserved for documentation.".to_string(),
                    attributes: BTreeMap::from([("zone".to_string(), json!("main"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.iana.org/help/example-domains".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b4".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rfooter:link:privacy".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Privacy Policy".to_string(),
                    attributes: BTreeMap::from([
                        ("href".to_string(), json!("/privacy")),
                        ("zone".to_string(), json!("footer")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://www.iana.org/help/example-domains".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > footer > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        assert_eq!(
            render_main_read_view_markdown(&snapshot),
            "# Example Domains\n\nAs described in RFC 2606 and RFC 6761, example domains are reserved for documentation."
        );
    }

    #[test]
    fn normalizes_prefixed_list_items_in_read_view() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "fixture://research/static-docs/list-cleanup".to_string(),
                source_type: SourceType::Fixture,
                title: Some("List Cleanup".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 256,
                estimated_tokens: 24,
                emitted_tokens: 24,
                truncated: false,
            },
            blocks: vec![SnapshotBlock {
                version: CONTRACT_VERSION.to_string(),
                id: "b1".to_string(),
                kind: SnapshotBlockKind::List,
                stable_ref: "rmain:list:cleanup".to_string(),
                role: SnapshotBlockRole::Content,
                text: "- Already prefixed item\n2. Numbered entry".to_string(),
                attributes: BTreeMap::new(),
                evidence: SnapshotEvidence {
                    source_url: "fixture://research/static-docs/list-cleanup".to_string(),
                    source_type: SourceType::Fixture,
                    dom_path_hint: Some("html > body > main > ul".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        assert_eq!(
            render_read_view_markdown(&snapshot),
            "- Already prefixed item\n- Numbered entry"
        );
    }

    #[test]
    fn infers_main_only_boundaries_from_dom_path_when_zone_is_missing() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://en.wikipedia.org/wiki/Jazz".to_string(),
                source_type: SourceType::Http,
                title: Some("Jazz".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rnav:link:portal".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Contents".to_string(),
                    attributes: BTreeMap::from([("href".to_string(), json!("#contents"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Jazz".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > nav > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:title".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Jazz".to_string(),
                    attributes: BTreeMap::from([("level".to_string(), json!(1))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Jazz".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:intro".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Jazz originated in African-American communities in New Orleans."
                        .to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Jazz".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > main > p".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b4".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rfooter:link:privacy".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Privacy policy".to_string(),
                    attributes: BTreeMap::from([(
                        "href".to_string(),
                        json!("/wiki/Privacy_policy"),
                    )]),
                    evidence: SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Jazz".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some("html > body > footer > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let markdown = render_main_read_view_markdown(&snapshot);

        assert!(markdown.contains("# Jazz"));
        assert!(markdown.contains("Jazz originated in African-American communities"));
        assert!(!markdown.contains("Contents"));
        assert!(!markdown.contains("Privacy policy"));
    }

    #[test]
    fn infers_docs_content_wrappers_as_main_content() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                source_type: SourceType::Http,
                title: Some("Quickstart | Firecrawl".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rbody:link:docs-home".to_string(),
                    role: SnapshotBlockRole::Cta,
                    text: "Firecrawl Docs home page".to_string(),
                    attributes: BTreeMap::from([(
                        "href".to_string(),
                        json!("https://firecrawl.dev"),
                    )]),
                    evidence: SnapshotEvidence {
                        source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.relative.antialiased > div.max-lg-contents.lg-flex > div.max-lg-contents.lg-flex-1 > div#navbar > div.flex.items-center > a"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::List,
                    stable_ref: "rbody:list:sidebar-group".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "- Quickstart - Skill + CLI - MCP Server".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.relative.antialiased > div.max-lg-contents.lg-flex > div.max-lg-contents.lg-flex-1 > div#body-content > div#sidebar-content.hidden.sticky > div#navigation-items > div.text-sm.relative > div.mt-6"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::List,
                    stable_ref: "rbody:list:table-of-contents-content".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "- Scrape your first website - More capabilities".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.relative.antialiased > div.max-lg-contents.lg-flex > div.max-lg-contents.lg-flex-1 > div#body-content > div.w-full > div#content-container.px-5.lg-pr-10 > div#content-area.grow.w-full > div#content.mdx-content.container-columns-container > nav#toc.table-of-contents"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b4".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rbody:link:scrape-your-first-website".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Scrape your first website".to_string(),
                    attributes: BTreeMap::from([("href".to_string(), json!("#scrape-your-first-website"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.relative.antialiased > div.max-lg-contents.lg-flex > div.max-lg-contents.lg-flex-1 > div#body-content > div.w-full > div#content-container.px-5.lg-pr-10 > div#content-area.grow.w-full > div#content.mdx-content.container-columns-container > nav#toc.table-of-contents > a"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b5".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rbody:heading:scrape-your-first-website".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Scrape your first website".to_string(),
                    attributes: BTreeMap::from([("level".to_string(), json!(1))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.relative.antialiased > div.max-lg-contents.lg-flex > div.max-lg-contents.lg-flex-1 > div#body-content > div.w-full > div#content-container.px-5.lg-pr-10 > div#content-area.grow.w-full > div#content.mdx-content.container-columns-container > h1"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b6".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rbody:text:intro".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Use Firecrawl with AI agents to scrape, search, and extract structured content from documentation.".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://docs.firecrawl.dev/introduction".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.relative.antialiased > div.max-lg-contents.lg-flex > div.max-lg-contents.lg-flex-1 > div#body-content > div.w-full > div#content-container.px-5.lg-pr-10 > div#content-area.grow.w-full > div#content.mdx-content.container-columns-container > p"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let markdown = render_main_read_view_markdown(&snapshot);
        assert!(markdown.contains("# Scrape your first website"));
        assert!(markdown.contains("Use Firecrawl with AI agents"));
        assert!(!markdown.contains("Firecrawl Docs home page"));
        assert!(!markdown.contains("Skill + CLI"));
        assert!(!markdown.contains("More capabilities"));

        let reading_compact = render_reading_compact_snapshot(&snapshot);
        assert!(reading_compact.contains("Scrape your first website"));
        assert!(!reading_compact.contains("Firecrawl Docs home page"));
        assert!(!reading_compact.contains("Skill + CLI"));
        assert!(!reading_compact.contains("More capabilities"));
    }

    #[test]
    fn infers_markdown_prose_wrappers_as_main_content() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://reactrouter.com/home".to_string(),
                source_type: SourceType::Http,
                title: Some("React Router Home".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rbody:link:api-reference".to_string(),
                    role: SnapshotBlockRole::Cta,
                    text: "API Reference".to_string(),
                    attributes: BTreeMap::from([(
                        "href".to_string(),
                        json!("https://api.reactrouter.com/v7/"),
                    )]),
                    evidence: SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.header-height-theme-spacing-16 > div.sticky.top-0 > div.relative.z-20 > div.flex.items-center > a"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rbody:heading:react-router-home".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "React Router Home".to_string(),
                    attributes: BTreeMap::from([("level".to_string(), json!(1))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.block.lg-flex > div.xl-flex.xl-w-full > div.min-w-0.px-4 > div.markdown.w-full > div.md-prose > h1"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rbody:text:intro".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "React Router is a multi-strategy router for React bridging the gap from React 18 to React 19.".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html > body > div.block.lg-flex > div.xl-flex.xl-w-full > div.min-w-0.px-4 > div.markdown.w-full > div.md-prose > p"
                                .to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let markdown = render_main_read_view_markdown(&snapshot);
        assert!(markdown.contains("# React Router Home"));
        assert!(markdown.contains("multi-strategy router for React"));
        assert!(!markdown.contains("API Reference"));
    }

    #[test]
    fn app_like_main_surfaces_drop_utility_links_from_main_only_outputs() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://firecrawl.dev/playground".to_string(),
                source_type: SourceType::Playwright,
                title: Some("Firecrawl Playground".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 48,
                emitted_tokens: 48,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:docs".to_string(),
                    role: SnapshotBlockRole::Cta,
                    text: "Docs".to_string(),
                    attributes: BTreeMap::from([
                        ("href".to_string(), json!("https://firecrawl.dev/docs")),
                        ("zone".to_string(), json!("main")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://firecrawl.dev/playground".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > div.hero > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:pricing".to_string(),
                    role: SnapshotBlockRole::SecondaryNav,
                    text: "Pricing".to_string(),
                    attributes: BTreeMap::from([
                        ("href".to_string(), json!("https://firecrawl.dev/pricing")),
                        ("zone".to_string(), json!("main")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://firecrawl.dev/playground".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > div.hero > nav > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:playground".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Playground".to_string(),
                    attributes: BTreeMap::from([
                        ("level".to_string(), json!(1)),
                        ("zone".to_string(), json!("main")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://firecrawl.dev/playground".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > section > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b4".to_string(),
                    kind: SnapshotBlockKind::Button,
                    stable_ref: "rmain:button:start-scraping".to_string(),
                    role: SnapshotBlockRole::FormControl,
                    text: "Start scraping".to_string(),
                    attributes: BTreeMap::from([("zone".to_string(), json!("main"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://firecrawl.dev/playground".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > form > button".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b5".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:github".to_string(),
                    role: SnapshotBlockRole::Supporting,
                    text: "GitHub".to_string(),
                    attributes: BTreeMap::from([
                        (
                            "href".to_string(),
                            json!("https://github.com/mendableai/firecrawl"),
                        ),
                        ("zone".to_string(), json!("main")),
                    ]),
                    evidence: SnapshotEvidence {
                        source_url: "https://firecrawl.dev/playground".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > div.social > a".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let markdown = render_main_read_view_markdown(&snapshot);
        assert!(markdown.contains("# Playground"));
        assert!(!markdown.contains("Docs"));
        assert!(!markdown.contains("Pricing"));
        assert!(!markdown.contains("Start scraping"));
        assert!(!markdown.contains("GitHub"));

        let reading_compact = render_reading_compact_snapshot(&snapshot);
        assert_eq!(reading_compact, "h1 Playground");
    }

    #[test]
    fn app_like_main_surfaces_fall_back_to_title_when_only_controls_remain() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://play.tailwindcss.com/".to_string(),
                source_type: SourceType::Playwright,
                title: Some("Tailwind Play".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 24,
                emitted_tokens: 24,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Button,
                    stable_ref: "rmain:button:share".to_string(),
                    role: SnapshotBlockRole::FormControl,
                    text: "Share".to_string(),
                    attributes: BTreeMap::from([("zone".to_string(), json!("main"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://play.tailwindcss.com/".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some(
                            "html > body > main > div.toolbar > button".to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Button,
                    stable_ref: "rmain:button:format".to_string(),
                    role: SnapshotBlockRole::FormControl,
                    text: "Format".to_string(),
                    attributes: BTreeMap::from([("zone".to_string(), json!("main"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://play.tailwindcss.com/".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some(
                            "html > body > main > div.toolbar > button:nth-of-type(2)".to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Button,
                    stable_ref: "rmain:button:download".to_string(),
                    role: SnapshotBlockRole::FormControl,
                    text: "Download".to_string(),
                    attributes: BTreeMap::from([("zone".to_string(), json!("main"))]),
                    evidence: SnapshotEvidence {
                        source_url: "https://play.tailwindcss.com/".to_string(),
                        source_type: SourceType::Playwright,
                        dom_path_hint: Some(
                            "html > body > main > div.toolbar > button:nth-of-type(3)".to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        assert_eq!(render_main_read_view_markdown(&snapshot), "# Tailwind Play");
        assert_eq!(
            render_reading_compact_snapshot(&snapshot),
            "h1 Tailwind Play"
        );
    }

    #[test]
    fn ignores_global_html_toc_markers_when_inferring_main_only_boundaries() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "https://zh.wikipedia.org/wiki/%E4%B8%AD%E5%9B%BD".to_string(),
                source_type: SourceType::Http,
                title: Some("中國".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:firstheading".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "中國".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://zh.wikipedia.org/wiki/%E4%B8%AD%E5%9B%BD".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html.client-nojs.vector-toc-available > body.skin-vector > div.mw-page-container > main#content.mw-body > header.mw-body-header.vector-page-titlebar".to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:intro".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "中國位於東亞。".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://zh.wikipedia.org/wiki/%E4%B8%AD%E5%9B%BD".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html.client-nojs.vector-toc-available > body.skin-vector > div.mw-page-container > div.mw-content-container > main#content.mw-body > div#mw-content-text > div.mw-parser-output > p".to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b3".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rnav:link:toc".to_string(),
                    role: SnapshotBlockRole::PrimaryNav,
                    text: "目录".to_string(),
                    attributes: BTreeMap::new(),
                    evidence: SnapshotEvidence {
                        source_url: "https://zh.wikipedia.org/wiki/%E4%B8%AD%E5%9B%BD".to_string(),
                        source_type: SourceType::Http,
                        dom_path_hint: Some(
                            "html.client-nojs.vector-toc-available > body.skin-vector > div.mw-page-container > div.vector-column-start > nav#mw-panel-toc.mw-table-of-contents-container > div#vector-toc.vector-toc".to_string(),
                        ),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let markdown = render_main_read_view_markdown(&snapshot);

        assert!(markdown.contains("# 中國"));
        assert!(markdown.contains("中國位於東亞。"));
        assert!(!markdown.contains("目录"));
    }

    #[test]
    fn renders_session_synthesis_markdown_with_citations() {
        let report = SessionSynthesisReport {
            version: CONTRACT_VERSION.to_string(),
            session_id: "session-001".to_string(),
            generated_at: "2026-03-14T00:00:00+09:00".to_string(),
            snapshot_count: 2,
            evidence_report_count: 1,
            visited_urls: vec!["https://example.com/docs".to_string()],
            working_set_refs: vec!["rmain:text:example".to_string()],
            synthesized_notes: vec!["Key note".to_string()],
            supported_claims: vec![SessionSynthesisClaim {
                version: CONTRACT_VERSION.to_string(),
                claim_id: "c1".to_string(),
                statement: "Example claim".to_string(),
                status: SessionSynthesisClaimStatus::EvidenceSupported,
                snapshot_ids: vec!["snap-1".to_string()],
                support_refs: vec!["rmain:text:example".to_string()],
                citations: vec![touch_browser_contracts::EvidenceCitation {
                    url: "https://example.com/docs".to_string(),
                    retrieved_at: "2026-03-14T00:00:00+09:00".to_string(),
                    source_type: SourceType::Http,
                    source_risk: SourceRisk::Low,
                    source_label: Some("Example".to_string()),
                }],
            }],
            contradicted_claims: Vec::new(),
            unsupported_claims: Vec::new(),
            needs_more_browsing_claims: Vec::new(),
        };

        let markdown = render_session_synthesis_markdown(&report);
        assert!(markdown.contains("# Session Synthesis"));
        assert!(markdown.contains("## Evidence-Supported Claims"));
        assert!(markdown.contains("Citations: https://example.com/docs"));
    }
}
