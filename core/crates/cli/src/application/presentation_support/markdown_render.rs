use serde_json::Value;
use touch_browser_contracts::{SnapshotBlock, SnapshotBlockKind};

pub(super) fn render_markdown_block(block: &SnapshotBlock) -> String {
    let text = normalize_markdown_text(&block.text);
    if text.is_empty() {
        return String::new();
    }

    match block.kind {
        SnapshotBlockKind::Heading => format!(
            "{} {}",
            "#".repeat(
                block
                    .attributes
                    .get("level")
                    .and_then(Value::as_u64)
                    .unwrap_or(1)
                    .clamp(1, 6) as usize,
            ),
            text
        ),
        SnapshotBlockKind::Text | SnapshotBlockKind::Metadata => text,
        SnapshotBlockKind::List => render_markdown_list(&text),
        SnapshotBlockKind::Table => render_markdown_table(block, &text),
        SnapshotBlockKind::Link => render_markdown_link(block, &text),
        SnapshotBlockKind::Button => format!("- {text}"),
        SnapshotBlockKind::Form | SnapshotBlockKind::Input => String::new(),
    }
}

fn normalize_markdown_text(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_markdown_list(text: &str) -> String {
    let items = text
        .lines()
        .map(normalize_list_item)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if items.len() <= 1 {
        return format!("- {}", normalize_list_item(text));
    }

    items
        .into_iter()
        .map(|item| format!("- {item}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn normalize_list_item(text: &str) -> String {
    let trimmed = text.trim();

    if let Some(stripped) = trimmed.strip_prefix("- ") {
        return stripped.trim().to_string();
    }
    if let Some(stripped) = trimmed.strip_prefix("* ") {
        return stripped.trim().to_string();
    }

    let mut chars = trimmed.chars().peekable();
    let mut digit_count = 0usize;
    while matches!(chars.peek(), Some(ch) if ch.is_ascii_digit()) {
        chars.next();
        digit_count += 1;
    }
    if digit_count > 0 && matches!(chars.peek(), Some('.' | ')')) {
        chars.next();
        if matches!(chars.peek(), Some(' ')) {
            chars.next();
            let rest = chars.collect::<String>().trim().to_string();
            if !rest.is_empty() {
                return rest;
            }
        }
    }

    trimmed.to_string()
}

fn render_markdown_table(block: &SnapshotBlock, text: &str) -> String {
    let rows = block.attributes.get("rows").and_then(Value::as_u64);
    let columns = block.attributes.get("columns").and_then(Value::as_u64);
    let label = match (rows, columns) {
        (Some(rows), Some(columns)) => format!("Table ({rows} rows x {columns} columns)"),
        (Some(rows), None) => format!("Table ({rows} rows)"),
        (None, Some(columns)) => format!("Table ({columns} columns)"),
        (None, None) => "Table".to_string(),
    };

    format!("{label}\n{text}")
}

fn render_markdown_link(block: &SnapshotBlock, text: &str) -> String {
    match block.attributes.get("href").and_then(Value::as_str) {
        Some(href) => format!("- [{text}]({href})"),
        None => format!("- {text}"),
    }
}
