use touch_browser_contracts::{SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole};

use super::layout_zones::{block_layout_zone, LayoutZone};

pub(super) fn keep_compact_block(block: &SnapshotBlock, has_heading: bool) -> bool {
    match block.kind {
        SnapshotBlockKind::Metadata => !has_heading,
        SnapshotBlockKind::Heading | SnapshotBlockKind::Table => true,
        SnapshotBlockKind::Text => is_salient_text_block(&block.text),
        SnapshotBlockKind::List => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) || is_salient_text_block(&block.text)
        }
        SnapshotBlockKind::Link => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Cta
            ) && is_salient_text_block(&block.text)
        }
        SnapshotBlockKind::Button => {
            matches!(block.role, SnapshotBlockRole::Cta) && is_salient_text_block(&block.text)
        }
        _ => false,
    }
}

pub(super) fn keep_reading_block(block: &SnapshotBlock, has_heading: bool) -> bool {
    if is_toc_like_block(block) {
        return false;
    }

    if has_heading && matches!(block.kind, SnapshotBlockKind::Metadata) {
        return false;
    }

    match block.kind {
        SnapshotBlockKind::Heading | SnapshotBlockKind::Table => true,
        SnapshotBlockKind::Text => is_salient_text_block(&block.text),
        SnapshotBlockKind::List => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) || is_salient_text_block(&block.text)
        }
        SnapshotBlockKind::Link => {
            matches!(block.role, SnapshotBlockRole::Content) && is_salient_text_block(&block.text)
        }
        _ => false,
    }
}

pub(super) fn keep_main_reading_block(
    block: &SnapshotBlock,
    has_heading: bool,
    has_main_zone: bool,
) -> bool {
    if !keep_reading_block(block, has_heading) {
        return false;
    }

    let zone = block_layout_zone(block);
    if has_main_zone {
        return matches!(zone, Some(LayoutZone::Main));
    }

    !matches!(
        zone,
        Some(LayoutZone::Nav | LayoutZone::Aside | LayoutZone::Header | LayoutZone::Footer)
    )
}

pub(super) fn keep_app_main_reading_block(
    block: &SnapshotBlock,
    has_heading: bool,
    has_main_zone: bool,
) -> bool {
    if !keep_main_reading_block(block, has_heading, has_main_zone) {
        return false;
    }

    match block.kind {
        SnapshotBlockKind::Heading => true,
        SnapshotBlockKind::Text => block.text.trim().chars().count() >= 40,
        SnapshotBlockKind::List | SnapshotBlockKind::Table => {
            block.text.trim().chars().count() >= 80
        }
        SnapshotBlockKind::Link => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) && block.text.trim().chars().count() >= 80
        }
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Input => false,
    }
}

pub(super) fn keep_navigation_block(block: &SnapshotBlock) -> bool {
    if matches!(
        block.role,
        SnapshotBlockRole::PrimaryNav
            | SnapshotBlockRole::SecondaryNav
            | SnapshotBlockRole::Cta
            | SnapshotBlockRole::FormControl
    ) {
        return true;
    }

    matches!(
        block.kind,
        SnapshotBlockKind::Link | SnapshotBlockKind::Button | SnapshotBlockKind::Input
    )
}

pub(super) fn keep_read_view_block(block: &SnapshotBlock, has_heading: bool) -> bool {
    if is_toc_like_block(block) {
        return false;
    }

    if has_heading && matches!(block.kind, SnapshotBlockKind::Metadata) {
        return false;
    }

    if block.text.trim().is_empty() {
        return false;
    }

    match block.kind {
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Heading
        | SnapshotBlockKind::Text
        | SnapshotBlockKind::Table
        | SnapshotBlockKind::List => true,
        SnapshotBlockKind::Link => matches!(
            block.role,
            SnapshotBlockRole::Content | SnapshotBlockRole::Supporting | SnapshotBlockRole::Cta
        ),
        SnapshotBlockKind::Button => matches!(block.role, SnapshotBlockRole::Cta),
        SnapshotBlockKind::Form | SnapshotBlockKind::Input => false,
    }
}

pub(super) fn keep_main_read_view_block(
    block: &SnapshotBlock,
    has_heading: bool,
    has_main_zone: bool,
) -> bool {
    if !keep_read_view_block(block, has_heading) {
        return false;
    }

    let zone = block_layout_zone(block);
    if has_main_zone {
        return matches!(zone, Some(LayoutZone::Main));
    }

    !matches!(
        zone,
        Some(LayoutZone::Nav | LayoutZone::Aside | LayoutZone::Header | LayoutZone::Footer)
    )
}

pub(super) fn keep_app_main_read_view_block(
    block: &SnapshotBlock,
    has_heading: bool,
    has_main_zone: bool,
) -> bool {
    if !keep_main_read_view_block(block, has_heading, has_main_zone) {
        return false;
    }

    match block.kind {
        SnapshotBlockKind::Heading => true,
        SnapshotBlockKind::Metadata => !has_heading && block.text.trim().chars().count() >= 3,
        SnapshotBlockKind::Text => block.text.trim().chars().count() >= 40,
        SnapshotBlockKind::List | SnapshotBlockKind::Table => {
            block.text.trim().chars().count() >= 80
        }
        SnapshotBlockKind::Link => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) && block.text.trim().chars().count() >= 80
        }
        SnapshotBlockKind::Button | SnapshotBlockKind::Form | SnapshotBlockKind::Input => false,
    }
}

fn is_salient_text_block(text: &str) -> bool {
    let word_count = text.split_whitespace().count();
    let lowered = text.to_ascii_lowercase();

    word_count <= 10
        || text.chars().any(|character| character.is_ascii_digit())
        || text.contains('$')
        || text.contains('%')
        || lowered.contains("rfc")
}

fn is_toc_like_block(block: &SnapshotBlock) -> bool {
    let stable_ref = block.stable_ref.to_ascii_lowercase();
    let dom_path = block
        .evidence
        .dom_path_hint
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let text = block.text.trim().to_ascii_lowercase();

    if stable_ref.contains("table-of-contents")
        || stable_ref.contains(":toc")
        || dom_path.contains("table-of-contents")
        || dom_path.contains("#toc")
        || dom_path.contains("toc.")
        || text == "contents"
        || text == "table of contents"
    {
        return true;
    }

    matches!(block.kind, SnapshotBlockKind::Link)
        && block
            .attributes
            .get("href")
            .and_then(|value| value.as_str())
            .is_some_and(|href| href.starts_with('#'))
}
