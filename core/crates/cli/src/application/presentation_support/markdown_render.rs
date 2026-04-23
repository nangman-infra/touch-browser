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
        SnapshotBlockKind::List => render_markdown_list(block, &text),
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

fn render_markdown_list(block: &SnapshotBlock, text: &str) -> String {
    let ordered = block
        .attributes
        .get("ordered")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let items = text
        .lines()
        .flat_map(|line| split_inline_list_items(line, ordered))
        .map(|item| normalize_list_item(&item))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();

    if items.len() <= 1 {
        let item = normalize_list_item(text);
        return if ordered {
            format!("1. {item}")
        } else {
            format!("- {item}")
        };
    }

    items
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            if ordered {
                format!("{}. {item}", index + 1)
            } else {
                format!("- {item}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn split_inline_list_items(text: &str, ordered: bool) -> Vec<String> {
    if ordered {
        split_inline_ordered_items(text)
    } else {
        split_inline_unordered_items(text)
    }
}

fn split_inline_ordered_items(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    let mut starts = Vec::new();
    let bytes = trimmed.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if index == 0 || bytes[index.saturating_sub(1)].is_ascii_whitespace() {
            let digit_start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            if index > digit_start
                && index + 1 < bytes.len()
                && matches!(bytes[index], b'.' | b')')
                && bytes[index + 1].is_ascii_whitespace()
            {
                starts.push(digit_start);
            }
        }
        index += 1;
    }

    split_by_starts(trimmed, starts)
}

fn split_inline_unordered_items(text: &str) -> Vec<String> {
    let trimmed = text.trim();
    if !trimmed.starts_with("- ") && !trimmed.starts_with("* ") {
        return vec![trimmed.to_string()];
    }

    let marker = if trimmed.starts_with("- ") {
        "- "
    } else {
        "* "
    };
    let starts = trimmed
        .match_indices(marker)
        .filter_map(|(index, _)| {
            if index == 0 || trimmed.as_bytes()[index - 1].is_ascii_whitespace() {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    split_by_starts(trimmed, starts)
}

fn split_by_starts(text: &str, starts: Vec<usize>) -> Vec<String> {
    if starts.len() <= 1 {
        return vec![text.to_string()];
    }

    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts.get(index + 1).copied().unwrap_or(text.len());
            text[*start..end].trim().to_string()
        })
        .filter(|item| !item.is_empty())
        .collect()
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
