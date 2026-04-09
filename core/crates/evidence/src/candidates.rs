use touch_browser_contracts::{SnapshotBlock, SnapshotBlockKind};

use crate::segmentation::segment_block_text;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CandidateText {
    pub(crate) index: usize,
    pub(crate) text: String,
}

pub(crate) fn block_candidates(block: &SnapshotBlock) -> Vec<CandidateText> {
    let raw_segments = match block.kind {
        SnapshotBlockKind::Text
        | SnapshotBlockKind::List
        | SnapshotBlockKind::Table
        | SnapshotBlockKind::Metadata => segment_block_text(&block.text),
        _ => vec![block.text.trim().to_string()],
    };

    let candidates = raw_segments
        .into_iter()
        .filter_map(|text| {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .enumerate()
        .map(|(index, text)| CandidateText { index, text })
        .collect::<Vec<_>>();

    if candidates.is_empty() {
        vec![CandidateText {
            index: 0,
            text: block.text.trim().to_string(),
        }]
    } else {
        candidates
    }
}

#[cfg(test)]
mod tests {
    use touch_browser_contracts::{
        SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotEvidence, SourceType,
    };

    use super::block_candidates;

    fn text_block(text: &str) -> SnapshotBlock {
        SnapshotBlock {
            version: "1.0.0".to_string(),
            id: "b1".to_string(),
            kind: SnapshotBlockKind::Text,
            stable_ref: "rmain:text:test".to_string(),
            role: SnapshotBlockRole::Content,
            text: text.to_string(),
            attributes: Default::default(),
            evidence: SnapshotEvidence {
                source_url: "https://example.com".to_string(),
                source_type: SourceType::Http,
                dom_path_hint: Some("html > body > main > p".to_string()),
                byte_range_start: None,
                byte_range_end: None,
            },
        }
    }

    #[test]
    fn splits_mixed_default_and_maximum_statements_into_distinct_candidates() {
        let block = text_block(
            "The default value for this setting is 3 seconds, but you can adjust this in increments of 1 second up to a maximum value of 900 seconds (15 minutes).",
        );

        let candidates = block_candidates(&block);
        let texts = candidates
            .into_iter()
            .map(|candidate| candidate.text)
            .collect::<Vec<_>>();

        assert_eq!(texts.len(), 2);
        assert!(texts[0].contains("default value"));
        assert!(texts[1].contains("maximum value"));
    }
}
