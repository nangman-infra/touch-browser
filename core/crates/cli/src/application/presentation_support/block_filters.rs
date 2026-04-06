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

fn is_salient_text_block(text: &str) -> bool {
    let word_count = text.split_whitespace().count();
    let lowered = text.to_ascii_lowercase();

    word_count <= 10
        || text.chars().any(|character| character.is_ascii_digit())
        || text.contains('$')
        || text.contains('%')
        || lowered.contains("rfc")
}
