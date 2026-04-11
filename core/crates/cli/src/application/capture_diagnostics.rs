use touch_browser_contracts::{
    CaptureDiagnostics, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotDocument,
    SourceType,
};

use super::deps::BrowserLoadDiagnostics;

const JS_PLACEHOLDER_HINTS: &[&str] = &[
    "enable javascript",
    "requires javascript",
    "javascript to run this app",
    "turn javascript on",
    "javascript is disabled",
    "you need to enable javascript",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CaptureSurface {
    Open,
    ReadView,
    Extract,
    SessionRefresh,
    Follow,
    Click,
    Type,
}

#[derive(Debug, Clone)]
struct SnapshotQualityAssessment {
    fallback_reason: Option<&'static str>,
    quality_score: f64,
    quality_label: &'static str,
    meaningful_block_count: usize,
    main_block_count: usize,
    shell_block_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct SnapshotQualityInputs {
    placeholder_detected: bool,
    meaningful_block_count: usize,
    main_block_count: usize,
    meaningful_chars: usize,
    longform_blocks: usize,
    shell_block_count: usize,
    content_headings: usize,
    text_like_blocks: usize,
    truncated: bool,
}

pub(crate) fn browser_fallback_reason(snapshot: &SnapshotDocument) -> Option<&'static str> {
    snapshot_quality_assessment(snapshot).fallback_reason
}

pub(crate) fn http_capture_diagnostics(
    snapshot: &SnapshotDocument,
    requested_budget: usize,
    surface: CaptureSurface,
) -> CaptureDiagnostics {
    build_capture_diagnostics(
        snapshot,
        requested_budget,
        "http",
        false,
        None,
        Some("http-fetch"),
        None,
        surface,
        None,
        None,
    )
}

pub(crate) fn browser_capture_diagnostics(
    snapshot: &SnapshotDocument,
    requested_budget: usize,
    fallback_triggered: bool,
    fallback_reason: Option<&str>,
    load_diagnostics: &BrowserLoadDiagnostics,
    surface: CaptureSurface,
) -> CaptureDiagnostics {
    build_capture_diagnostics(
        snapshot,
        requested_budget,
        if fallback_triggered {
            "browser-fallback"
        } else {
            "browser"
        },
        fallback_triggered,
        fallback_reason,
        Some(load_diagnostics.wait_strategy.as_str()),
        Some(load_diagnostics),
        surface,
        None,
        None,
    )
}

pub(crate) fn browser_action_diagnostics(
    snapshot: &SnapshotDocument,
    requested_budget: usize,
    load_diagnostics: &BrowserLoadDiagnostics,
    surface: CaptureSurface,
    target_ref: &str,
    sensitive: Option<bool>,
) -> CaptureDiagnostics {
    build_capture_diagnostics(
        snapshot,
        requested_budget,
        "browser",
        false,
        None,
        Some(load_diagnostics.wait_strategy.as_str()),
        Some(load_diagnostics),
        surface,
        Some(target_ref),
        sensitive,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_capture_diagnostics(
    snapshot: &SnapshotDocument,
    requested_budget: usize,
    capture_mode: &str,
    fallback_triggered: bool,
    fallback_reason: Option<&str>,
    wait_strategy: Option<&str>,
    load_diagnostics: Option<&BrowserLoadDiagnostics>,
    surface: CaptureSurface,
    target_ref: Option<&str>,
    sensitive: Option<bool>,
) -> CaptureDiagnostics {
    let assessment = snapshot_quality_assessment(snapshot);
    CaptureDiagnostics {
        requested_budget,
        effective_budget: snapshot.budget.requested_tokens,
        capture_mode: capture_mode.to_string(),
        surface: capture_surface_label(surface).to_string(),
        fallback_triggered,
        fallback_reason: fallback_reason.map(ToString::to_string),
        wait_strategy: wait_strategy.unwrap_or("none").to_string(),
        wait_budget_ms: load_diagnostics.and_then(|item| item.wait_budget_ms),
        wait_consumed_ms: load_diagnostics.and_then(|item| item.wait_consumed_ms),
        wait_stop_reason: load_diagnostics.and_then(|item| item.wait_stop_reason.clone()),
        quality_score: assessment.quality_score,
        quality_label: assessment.quality_label.to_string(),
        meaningful_block_count: assessment.meaningful_block_count,
        main_block_count: assessment.main_block_count,
        shell_block_count: assessment.shell_block_count,
        truncated: snapshot.budget.truncated,
        recommended_next_step: recommended_next_step(snapshot, surface, &assessment).to_string(),
        target_ref: target_ref.map(ToString::to_string),
        sensitive,
    }
}

fn capture_surface_label(surface: CaptureSurface) -> &'static str {
    match surface {
        CaptureSurface::Open => "open",
        CaptureSurface::ReadView => "read-view",
        CaptureSurface::Extract => "extract",
        CaptureSurface::SessionRefresh => "session-refresh",
        CaptureSurface::Follow => "follow",
        CaptureSurface::Click => "click",
        CaptureSurface::Type => "type",
    }
}

fn recommended_next_step(
    snapshot: &SnapshotDocument,
    surface: CaptureSurface,
    assessment: &SnapshotQualityAssessment,
) -> &'static str {
    if snapshot.budget.truncated {
        return "increase-budget";
    }

    if snapshot.source.source_type == SourceType::Http && assessment.fallback_reason.is_some() {
        return "retry-browser";
    }

    match surface {
        CaptureSurface::Open => "use-read-view",
        CaptureSurface::SessionRefresh => "use-read-view",
        CaptureSurface::Follow => "use-read-view",
        CaptureSurface::ReadView => {
            if assessment.quality_score < 0.45 {
                "continue"
            } else {
                "use-extract"
            }
        }
        CaptureSurface::Click | CaptureSurface::Type => "continue",
        CaptureSurface::Extract => "continue",
    }
}

fn snapshot_quality_assessment(snapshot: &SnapshotDocument) -> SnapshotQualityAssessment {
    if snapshot.source.source_type != SourceType::Fixture && snapshot.blocks.is_empty() {
        return empty_snapshot_quality_assessment();
    }

    let quality_inputs = snapshot_quality_inputs(snapshot);
    let fallback_reason = snapshot_fallback_reason(snapshot, &quality_inputs);
    let quality_score = snapshot_quality_score(&quality_inputs);
    let quality_label = snapshot_quality_label(quality_score);

    SnapshotQualityAssessment {
        fallback_reason,
        quality_score: (quality_score * 100.0).round() / 100.0,
        quality_label,
        meaningful_block_count: quality_inputs.meaningful_block_count,
        main_block_count: quality_inputs.main_block_count,
        shell_block_count: quality_inputs.shell_block_count,
    }
}

fn empty_snapshot_quality_assessment() -> SnapshotQualityAssessment {
    SnapshotQualityAssessment {
        fallback_reason: Some("empty-snapshot"),
        quality_score: 0.0,
        quality_label: "low",
        meaningful_block_count: 0,
        main_block_count: 0,
        shell_block_count: 0,
    }
}

fn snapshot_quality_inputs(snapshot: &SnapshotDocument) -> SnapshotQualityInputs {
    let normalized_blocks = snapshot
        .blocks
        .iter()
        .map(|block| normalized_block_text(&block.text))
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>();
    let placeholder_detected = normalized_blocks
        .iter()
        .any(|text| JS_PLACEHOLDER_HINTS.iter().any(|hint| text.contains(hint)));
    let meaningful_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| is_meaningful_snapshot_block(block))
        .collect::<Vec<_>>();

    SnapshotQualityInputs {
        placeholder_detected,
        meaningful_block_count: meaningful_blocks.len(),
        main_block_count: meaningful_blocks
            .iter()
            .filter(|block| block.stable_ref.starts_with("rmain:"))
            .count(),
        meaningful_chars: meaningful_blocks
            .iter()
            .map(|block| block.text.trim().chars().count())
            .sum::<usize>(),
        longform_blocks: snapshot
            .blocks
            .iter()
            .filter(|block| is_longform_content_block(block))
            .count(),
        shell_block_count: snapshot
            .blocks
            .iter()
            .filter(|block| is_shell_like_block(block))
            .count(),
        content_headings: snapshot
            .blocks
            .iter()
            .filter(|block| {
                matches!(block.kind, SnapshotBlockKind::Heading)
                    && !matches!(
                        block.role,
                        SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav
                    )
                    && block.text.trim().chars().count() >= 4
            })
            .count(),
        text_like_blocks: snapshot
            .blocks
            .iter()
            .filter(|block| {
                matches!(
                    block.kind,
                    SnapshotBlockKind::Text | SnapshotBlockKind::List | SnapshotBlockKind::Table
                )
            })
            .count(),
        truncated: snapshot.budget.truncated,
    }
}

fn snapshot_fallback_reason(
    snapshot: &SnapshotDocument,
    quality_inputs: &SnapshotQualityInputs,
) -> Option<&'static str> {
    if snapshot.source.source_type != SourceType::Http {
        return None;
    }

    if quality_inputs.placeholder_detected {
        return Some("js-placeholder");
    }

    if quality_inputs.main_block_count == 0
        && quality_inputs.meaningful_block_count <= 2
        && quality_inputs.meaningful_chars < 240
    {
        return Some("missing-main-content");
    }

    let primary_shell_heavy = quality_inputs.longform_blocks == 0
        && quality_inputs.text_like_blocks <= 1
        && quality_inputs.shell_block_count >= 8;
    let secondary_shell_heavy = quality_inputs.longform_blocks <= 1
        && quality_inputs.meaningful_chars < 320
        && quality_inputs.shell_block_count >= 10
        && quality_inputs.content_headings <= 1;

    if primary_shell_heavy || secondary_shell_heavy {
        Some("shell-heavy")
    } else {
        None
    }
}

fn snapshot_quality_score(quality_inputs: &SnapshotQualityInputs) -> f64 {
    let content_score = ((quality_inputs.main_block_count as f64) * 0.12)
        + ((quality_inputs.longform_blocks as f64) * 0.16)
        + ((quality_inputs.content_headings as f64) * 0.08)
        + ((quality_inputs.meaningful_chars.min(2400) as f64) / 2400.0) * 0.34
        + ((quality_inputs.text_like_blocks.min(8) as f64) / 8.0) * 0.18;
    let shell_penalty = ((quality_inputs.shell_block_count.min(18) as f64) / 18.0)
        * if quality_inputs.main_block_count == 0 {
            0.36
        } else {
            0.22
        };
    let placeholder_penalty = if quality_inputs.placeholder_detected {
        0.45
    } else {
        0.0
    };
    let truncation_penalty = if quality_inputs.truncated { 0.08 } else { 0.0 };

    (content_score - shell_penalty - placeholder_penalty - truncation_penalty).clamp(0.0, 1.0)
}

fn snapshot_quality_label(quality_score: f64) -> &'static str {
    if quality_score >= 0.75 {
        "high"
    } else if quality_score >= 0.45 {
        "medium"
    } else {
        "low"
    }
}

fn normalized_block_text(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn is_meaningful_snapshot_block(block: &SnapshotBlock) -> bool {
    let char_count = block.text.trim().chars().count();
    match block.kind {
        SnapshotBlockKind::Heading => char_count >= 4,
        SnapshotBlockKind::Text => char_count >= 32,
        SnapshotBlockKind::List | SnapshotBlockKind::Table => char_count >= 24,
        SnapshotBlockKind::Link => block.stable_ref.starts_with("rmain:") && char_count >= 48,
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Input => false,
    }
}

fn is_longform_content_block(block: &SnapshotBlock) -> bool {
    let char_count = block.text.trim().chars().count();
    match block.kind {
        SnapshotBlockKind::Heading => {
            !matches!(
                block.role,
                SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav
            ) && char_count >= 8
        }
        SnapshotBlockKind::Text => char_count >= 80,
        SnapshotBlockKind::List | SnapshotBlockKind::Table => char_count >= 64,
        SnapshotBlockKind::Link => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) && char_count >= 72
        }
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Input => false,
    }
}

fn is_shell_like_block(block: &SnapshotBlock) -> bool {
    if matches!(
        block.role,
        SnapshotBlockRole::PrimaryNav
            | SnapshotBlockRole::SecondaryNav
            | SnapshotBlockRole::Cta
            | SnapshotBlockRole::FormControl
    ) {
        return true;
    }

    match block.kind {
        SnapshotBlockKind::Link
        | SnapshotBlockKind::Button
        | SnapshotBlockKind::Form
        | SnapshotBlockKind::Input => true,
        SnapshotBlockKind::List => block.text.split_whitespace().count() <= 12,
        SnapshotBlockKind::Metadata
        | SnapshotBlockKind::Heading
        | SnapshotBlockKind::Text
        | SnapshotBlockKind::Table => false,
    }
}
