use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use touch_browser_contracts::{EvidenceReport, SnapshotBlockKind, SnapshotDocument};

pub fn crate_status() -> &'static str {
    "memory ready"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDiff {
    pub from_snapshot_id: String,
    pub to_snapshot_id: String,
    pub added_refs: Vec<String>,
    pub removed_refs: Vec<String>,
    pub retained_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompactionResult {
    pub kept_refs: Vec<String>,
    pub dropped_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemoryTurn {
    pub turn_index: usize,
    pub snapshot_id: String,
    pub source_url: String,
    pub kept_refs: Vec<String>,
    pub dropped_refs: Vec<String>,
    pub supported_claim_ids: Vec<String>,
    pub note_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MemorySessionSummary {
    pub turn_count: usize,
    pub max_working_set_size: usize,
    pub final_working_set_size: usize,
    pub visited_urls: Vec<String>,
    pub synthesized_notes: Vec<String>,
}

pub fn diff_snapshots(
    from_snapshot_id: &str,
    from_snapshot: &SnapshotDocument,
    to_snapshot_id: &str,
    to_snapshot: &SnapshotDocument,
) -> SnapshotDiff {
    let left = from_snapshot
        .blocks
        .iter()
        .map(|block| block.stable_ref.clone())
        .collect::<BTreeSet<_>>();
    let right = to_snapshot
        .blocks
        .iter()
        .map(|block| block.stable_ref.clone())
        .collect::<BTreeSet<_>>();

    SnapshotDiff {
        from_snapshot_id: from_snapshot_id.to_string(),
        to_snapshot_id: to_snapshot_id.to_string(),
        added_refs: right.difference(&left).cloned().collect(),
        removed_refs: left.difference(&right).cloned().collect(),
        retained_refs: left.intersection(&right).cloned().collect(),
    }
}

pub fn compact_working_set(
    snapshot: &SnapshotDocument,
    evidence: Option<&EvidenceReport>,
    current_refs: &[String],
    limit: usize,
) -> CompactionResult {
    let mut ordered_refs = Vec::new();

    if let Some(report) = evidence {
        for claim in &report.supported_claims {
            for block_id in &claim.support {
                if let Some(block) = snapshot.blocks.iter().find(|block| &block.id == block_id) {
                    ordered_refs.push(block.stable_ref.clone());
                }
            }
        }
    }

    for block in &snapshot.blocks {
        if matches!(
            block.kind,
            SnapshotBlockKind::Heading
                | SnapshotBlockKind::Table
                | SnapshotBlockKind::List
                | SnapshotBlockKind::Metadata
        ) {
            ordered_refs.push(block.stable_ref.clone());
        }
    }

    for stable_ref in current_refs {
        if snapshot
            .blocks
            .iter()
            .any(|block| &block.stable_ref == stable_ref)
        {
            ordered_refs.push(stable_ref.clone());
        }
    }

    let mut seen = BTreeSet::new();
    let kept_refs = ordered_refs
        .into_iter()
        .filter(|stable_ref| seen.insert(stable_ref.clone()))
        .take(limit.max(1))
        .collect::<Vec<_>>();

    let kept_set = kept_refs.iter().cloned().collect::<BTreeSet<_>>();
    let dropped_refs = current_refs
        .iter()
        .filter(|stable_ref| !kept_set.contains(*stable_ref))
        .cloned()
        .collect::<Vec<_>>();

    CompactionResult {
        kept_refs,
        dropped_refs,
    }
}

pub fn plan_memory_turn(
    turn_index: usize,
    snapshot_id: &str,
    snapshot: &SnapshotDocument,
    evidence: Option<&EvidenceReport>,
    current_refs: &[String],
    limit: usize,
) -> MemoryTurn {
    let compaction = compact_working_set(snapshot, evidence, current_refs, limit);
    capture_memory_turn(
        turn_index,
        snapshot_id,
        snapshot,
        evidence,
        &compaction.kept_refs,
        &compaction.dropped_refs,
    )
}

pub fn capture_memory_turn(
    turn_index: usize,
    snapshot_id: &str,
    snapshot: &SnapshotDocument,
    evidence: Option<&EvidenceReport>,
    kept_refs: &[String],
    dropped_refs: &[String],
) -> MemoryTurn {
    MemoryTurn {
        turn_index,
        snapshot_id: snapshot_id.to_string(),
        source_url: snapshot.source.source_url.clone(),
        kept_refs: kept_refs.to_vec(),
        dropped_refs: dropped_refs.to_vec(),
        supported_claim_ids: evidence
            .map(|report| {
                report
                    .supported_claims
                    .iter()
                    .map(|claim| claim.claim_id.clone())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        note_lines: synthesize_note_lines(snapshot, evidence, 3),
    }
}

pub fn summarize_turns(turns: &[MemoryTurn], note_limit: usize) -> MemorySessionSummary {
    let max_working_set_size = turns
        .iter()
        .map(|turn| turn.kept_refs.len())
        .max()
        .unwrap_or(0);
    let final_working_set_size = turns.last().map(|turn| turn.kept_refs.len()).unwrap_or(0);
    let visited_urls = turns
        .iter()
        .map(|turn| turn.source_url.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();

    MemorySessionSummary {
        turn_count: turns.len(),
        max_working_set_size,
        final_working_set_size,
        visited_urls,
        synthesized_notes: rollup_notes(turns, note_limit),
    }
}

pub fn rollup_notes(turns: &[MemoryTurn], limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();

    turns
        .iter()
        .flat_map(|turn| turn.note_lines.iter())
        .filter(|line| seen.insert((*line).clone()))
        .take(limit.max(1))
        .cloned()
        .collect()
}

fn synthesize_note_lines(
    snapshot: &SnapshotDocument,
    evidence: Option<&EvidenceReport>,
    max_notes: usize,
) -> Vec<String> {
    let mut notes = Vec::new();

    if let Some(report) = evidence {
        for claim in &report.supported_claims {
            notes.push(claim.statement.clone());
        }
    }

    if notes.is_empty() {
        for block in &snapshot.blocks {
            if matches!(
                block.kind,
                SnapshotBlockKind::Heading | SnapshotBlockKind::Metadata | SnapshotBlockKind::Text
            ) {
                notes.push(block.text.clone());
            }
        }
    }

    let mut seen = BTreeSet::new();
    notes
        .into_iter()
        .filter(|note| !note.is_empty() && seen.insert(note.clone()))
        .take(max_notes.max(1))
        .collect()
}

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        EvidenceBlock, EvidenceCitation, EvidenceReport, EvidenceSource, SnapshotBlock,
        SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument, SnapshotEvidence,
        SnapshotSource, SourceRisk, SourceType, CONTRACT_VERSION, STABLE_REF_VERSION,
    };

    use super::{compact_working_set, diff_snapshots, plan_memory_turn, summarize_turns};

    #[test]
    fn computes_added_and_removed_refs() {
        let left = test_snapshot(vec![
            test_block(
                "b1",
                "rmain:heading:getting-started",
                SnapshotBlockKind::Heading,
            ),
            test_block("b2", "rmain:text:intro", SnapshotBlockKind::Text),
        ]);
        let right = test_snapshot(vec![
            test_block(
                "b1",
                "rmain:heading:getting-started",
                SnapshotBlockKind::Heading,
            ),
            test_block("b3", "rmain:table:pricing", SnapshotBlockKind::Table),
        ]);

        let diff = diff_snapshots("snap_left", &left, "snap_right", &right);

        assert_eq!(diff.added_refs, vec!["rmain:table:pricing"]);
        assert_eq!(diff.removed_refs, vec!["rmain:text:intro"]);
        assert_eq!(diff.retained_refs, vec!["rmain:heading:getting-started"]);
    }

    #[test]
    fn compacts_using_supported_claims_then_salient_blocks() {
        let snapshot = test_snapshot(vec![
            test_block("b1", "rhead:metadata:title", SnapshotBlockKind::Metadata),
            test_block(
                "b2",
                "rmain:heading:getting-started",
                SnapshotBlockKind::Heading,
            ),
            test_block("b3", "rmain:text:intro", SnapshotBlockKind::Text),
            test_block("b4", "rmain:table:pricing", SnapshotBlockKind::Table),
        ]);
        let evidence = EvidenceReport {
            version: CONTRACT_VERSION.to_string(),
            generated_at: "2026-03-14T00:00:00+09:00".to_string(),
            source: EvidenceSource {
                source_url: "fixture://test".to_string(),
                source_type: SourceType::Fixture,
                source_risk: SourceRisk::Low,
                source_label: Some("test".to_string()),
            },
            supported_claims: vec![EvidenceBlock {
                version: CONTRACT_VERSION.to_string(),
                claim_id: "c1".to_string(),
                statement: "Pricing exists.".to_string(),
                support: vec!["b4".to_string()],
                confidence: 0.9,
                citation: EvidenceCitation {
                    url: "fixture://test".to_string(),
                    retrieved_at: "2026-03-14T00:00:00+09:00".to_string(),
                    source_type: SourceType::Fixture,
                    source_risk: SourceRisk::Low,
                    source_label: Some("test".to_string()),
                },
            }],
            unsupported_claims: Vec::new(),
        };

        let compacted = compact_working_set(
            &snapshot,
            Some(&evidence),
            &["rmain:text:intro".to_string()],
            3,
        );

        assert_eq!(
            compacted.kept_refs,
            vec![
                "rmain:table:pricing".to_string(),
                "rhead:metadata:title".to_string(),
                "rmain:heading:getting-started".to_string(),
            ]
        );
        assert_eq!(compacted.dropped_refs, vec!["rmain:text:intro".to_string()]);
    }

    #[test]
    fn synthesizes_memory_notes_from_supported_claims() {
        let snapshot = test_snapshot(vec![
            test_block("b1", "rhead:metadata:title", SnapshotBlockKind::Metadata),
            test_block("b2", "rmain:heading:pricing", SnapshotBlockKind::Heading),
            test_block("b3", "rmain:text:intro", SnapshotBlockKind::Text),
            test_block("b4", "rmain:table:pricing", SnapshotBlockKind::Table),
        ]);
        let evidence = EvidenceReport {
            version: CONTRACT_VERSION.to_string(),
            generated_at: "2026-03-14T00:00:00+09:00".to_string(),
            source: EvidenceSource {
                source_url: "fixture://test".to_string(),
                source_type: SourceType::Fixture,
                source_risk: SourceRisk::Low,
                source_label: Some("test".to_string()),
            },
            supported_claims: vec![EvidenceBlock {
                version: CONTRACT_VERSION.to_string(),
                claim_id: "c1".to_string(),
                statement: "The Starter plan costs $29 per month.".to_string(),
                support: vec!["b4".to_string()],
                confidence: 0.9,
                citation: EvidenceCitation {
                    url: "fixture://test".to_string(),
                    retrieved_at: "2026-03-14T00:00:00+09:00".to_string(),
                    source_type: SourceType::Fixture,
                    source_risk: SourceRisk::Low,
                    source_label: Some("test".to_string()),
                },
            }],
            unsupported_claims: Vec::new(),
        };

        let turn = plan_memory_turn(1, "snap_1", &snapshot, Some(&evidence), &[], 3);
        let summary = summarize_turns(&[turn], 4);

        assert_eq!(summary.turn_count, 1);
        assert_eq!(summary.max_working_set_size, 3);
        assert_eq!(
            summary.synthesized_notes,
            vec!["The Starter plan costs $29 per month.".to_string()]
        );
    }

    #[test]
    fn summarizes_twenty_turn_memory_without_exceeding_limit() {
        let snapshot = test_snapshot(vec![
            test_block("b1", "rhead:metadata:title", SnapshotBlockKind::Metadata),
            test_block(
                "b2",
                "rmain:heading:getting-started",
                SnapshotBlockKind::Heading,
            ),
            test_block("b3", "rmain:text:intro", SnapshotBlockKind::Text),
            test_block("b4", "rmain:table:pricing", SnapshotBlockKind::Table),
        ]);

        let mut refs = Vec::new();
        let mut turns = Vec::new();
        for index in 0..20 {
            let turn = plan_memory_turn(
                index + 1,
                &format!("snap_{index}"),
                &snapshot,
                None,
                &refs,
                3,
            );
            refs = turn.kept_refs.clone();
            turns.push(turn);
        }

        let summary = summarize_turns(&turns, 4);

        assert_eq!(summary.turn_count, 20);
        assert_eq!(summary.max_working_set_size, 3);
        assert_eq!(summary.final_working_set_size, 3);
        assert!(summary
            .synthesized_notes
            .contains(&"rhead:metadata:title".to_string()));
    }

    fn test_snapshot(blocks: Vec<SnapshotBlock>) -> SnapshotDocument {
        SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "fixture://test".to_string(),
                source_type: SourceType::Fixture,
                title: Some("test".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 100,
                estimated_tokens: 100,
                emitted_tokens: 100,
                truncated: false,
            },
            blocks,
        }
    }

    fn test_block(id: &str, stable_ref: &str, kind: SnapshotBlockKind) -> SnapshotBlock {
        SnapshotBlock {
            version: CONTRACT_VERSION.to_string(),
            id: id.to_string(),
            kind,
            stable_ref: stable_ref.to_string(),
            role: SnapshotBlockRole::Content,
            text: stable_ref.to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "fixture://test".to_string(),
                source_type: SourceType::Fixture,
                dom_path_hint: None,
                byte_range_start: None,
                byte_range_end: None,
            },
        }
    }
}
