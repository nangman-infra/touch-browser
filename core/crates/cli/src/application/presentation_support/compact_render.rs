use serde_json::Value;
use touch_browser_contracts::{SnapshotBlock, SnapshotBlockKind};

pub(super) fn render_compact_block(block: &SnapshotBlock) -> String {
    let mut parts = vec![compact_kind_code(
        &block.kind,
        block.attributes.get("level").and_then(Value::as_u64),
    )];

    if let Some(attrs) = compact_attr_fragment(block) {
        parts.push(attrs);
    }

    let digest = compact_text_digest(&block.text, &block.kind);
    if !digest.is_empty() {
        parts.push(digest);
    }
    parts.join(" ")
}

fn compact_attr_fragment(block: &SnapshotBlock) -> Option<String> {
    match block.kind {
        SnapshotBlockKind::Link => block
            .attributes
            .get("href")
            .and_then(Value::as_str)
            .and_then(compact_href_fragment)
            .map(|fragment| format!("@{fragment}")),
        SnapshotBlockKind::Input => block
            .attributes
            .get("inputType")
            .and_then(Value::as_str)
            .map(|input_type| format!("={input_type}")),
        SnapshotBlockKind::Table => {
            let rows = block.attributes.get("rows").and_then(Value::as_u64);
            let columns = block.attributes.get("columns").and_then(Value::as_u64);
            match (rows, columns) {
                (Some(rows), Some(columns)) => Some(format!("{rows}x{columns}")),
                (Some(rows), None) => Some(format!("r{rows}")),
                (None, Some(columns)) => Some(format!("c{columns}")),
                (None, None) => None,
            }
        }
        _ => None,
    }
}

fn compact_href_fragment(href: &str) -> Option<String> {
    let trimmed = href.trim();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        return None;
    }

    let no_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let normalized = no_scheme.trim_matches('/').to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    normalized
        .split('/')
        .next()
        .map(str::trim)
        .filter(|fragment| !fragment.is_empty())
        .map(ToOwned::to_owned)
}

fn compact_text_digest(text: &str, kind: &SnapshotBlockKind) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        return String::new();
    }

    match kind {
        SnapshotBlockKind::Heading => normalized,
        SnapshotBlockKind::Link | SnapshotBlockKind::Button | SnapshotBlockKind::Input => {
            normalized
        }
        _ => normalized
            .split_whitespace()
            .take(12)
            .collect::<Vec<_>>()
            .join(" "),
    }
}

fn compact_kind_code(kind: &SnapshotBlockKind, level: Option<u64>) -> String {
    match kind {
        SnapshotBlockKind::Heading => format!("h{}", level.unwrap_or(1).clamp(1, 6)),
        SnapshotBlockKind::Text => "p".to_string(),
        SnapshotBlockKind::List => "l".to_string(),
        SnapshotBlockKind::Table => "t".to_string(),
        SnapshotBlockKind::Link => "a".to_string(),
        SnapshotBlockKind::Button => "b".to_string(),
        SnapshotBlockKind::Input => "i".to_string(),
        SnapshotBlockKind::Metadata => "m".to_string(),
        SnapshotBlockKind::Form => "f".to_string(),
    }
}
