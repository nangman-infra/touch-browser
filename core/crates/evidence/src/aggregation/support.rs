use std::collections::{BTreeMap, BTreeSet};

use touch_browser_contracts::SnapshotBlock;

use crate::{
    normalization::{normalize_text, token_overlap_ratio, tokenize_significant, tokens_match},
    scoring::{
        block_search_text, claim_token_weight, exact_match_bonus, is_narrative_aggregate_block,
        nearest_heading_context, numeric_overlap_ratio, primary_heading_context,
        weighted_token_overlap_ratio, ScoredCandidate, ScoringContext,
    },
};

pub(super) fn aggregate_support_score(
    normalized_claim: &str,
    claim_tokens: &[String],
    claim_anchor_tokens: &[String],
    claim_numeric_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    scoring_context: &ScoringContext,
) -> f64 {
    let narrative_support_count = top_support
        .iter()
        .filter(|candidate| is_narrative_aggregate_block(candidate.block))
        .count();
    let relevant_primary_heading = primary_heading_context(blocks)
        .filter(|heading| primary_heading_supports_claim(heading, claim_anchor_tokens));

    if narrative_support_count < 2
        && !(narrative_support_count >= 1 && relevant_primary_heading.is_some())
    {
        return 0.0;
    }

    let aggregated_text = aggregate_support_text(top_support, blocks);
    if aggregated_text.is_empty() {
        return 0.0;
    }

    let aggregated_tokens = tokenize_significant(&aggregated_text);
    if aggregated_tokens.is_empty() {
        return 0.0;
    }

    let lexical_overlap = weighted_token_overlap_ratio(
        claim_tokens,
        &aggregated_tokens,
        &scoring_context.claim_token_weights,
    );
    let exact_bonus = exact_match_bonus(normalized_claim, &normalize_text(&aggregated_text));
    let numeric_overlap = numeric_overlap_ratio(claim_numeric_tokens, &aggregated_text);
    let title_bonus = relevant_primary_heading.map(|_| 0.04).unwrap_or(0.0);
    let distributed_support_bonus =
        distributed_support_bonus(claim_anchor_tokens, top_support, scoring_context);
    let multi_block_context_bonus =
        multi_block_context_bonus(narrative_support_count, relevant_primary_heading.is_some());
    let support_density_bonus = support_density_bonus(top_support, narrative_support_count);

    ((lexical_overlap * 0.76)
        + (exact_bonus * 0.14)
        + (numeric_overlap * 0.06)
        + title_bonus
        + multi_block_context_bonus
        + support_density_bonus
        + distributed_support_bonus)
        .min(1.0)
}

pub(super) fn aggregate_support_text(
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
) -> String {
    let mut seen_blocks = BTreeSet::new();
    let mut parts = Vec::new();

    append_primary_heading_text(&mut seen_blocks, &mut parts, blocks);

    for candidate in top_support {
        append_candidate_support_text(&mut seen_blocks, &mut parts, blocks, candidate);
    }

    parts.join(" ")
}

fn append_unique_block_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    block: &SnapshotBlock,
) {
    if seen_blocks.insert(block.id.clone()) {
        parts.push(block_search_text(block));
    }
}

fn append_primary_heading_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
) {
    if let Some(primary_heading) = primary_heading_context(blocks) {
        append_unique_block_text(seen_blocks, parts, primary_heading);
    }
}

fn append_heading_context_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
    block: &SnapshotBlock,
) {
    if let Some(heading) = nearest_heading_context(blocks, block) {
        append_unique_block_text(seen_blocks, parts, heading);
    }
}

fn append_candidate_support_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
    candidate: &ScoredCandidate<'_>,
) {
    append_unique_block_text(seen_blocks, parts, candidate.block);
    append_heading_context_text(seen_blocks, parts, blocks, candidate.block);
}

fn distributed_support_bonus(
    claim_anchor_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
    scoring_context: &ScoringContext,
) -> f64 {
    if top_support.len() < 2 {
        return 0.0;
    }
    if claim_anchor_tokens.len() < 2 {
        return 0.0;
    }

    let mut covered_anchor_tokens = BTreeSet::new();
    let mut supporting_blocks = 0usize;

    for candidate in top_support {
        let block_tokens = tokenize_significant(&block_search_text(candidate.block));
        let matched = claim_anchor_tokens
            .iter()
            .filter(|claim_token| {
                block_tokens
                    .iter()
                    .any(|block_token| tokens_match(claim_token, block_token))
            })
            .cloned()
            .collect::<BTreeSet<_>>();

        if !matched.is_empty() {
            supporting_blocks += 1;
            covered_anchor_tokens.extend(matched);
        }
    }

    if supporting_blocks < 2 {
        return 0.0;
    }

    let coverage = weighted_anchor_coverage(
        claim_anchor_tokens,
        &covered_anchor_tokens,
        &scoring_context.claim_token_weights,
    );

    if coverage >= 0.8 {
        0.14
    } else if coverage >= 0.6 {
        0.10
    } else {
        0.0
    }
}

fn multi_block_context_bonus(
    narrative_support_count: usize,
    has_relevant_primary_heading: bool,
) -> f64 {
    if narrative_support_count >= 2 && has_relevant_primary_heading {
        0.06
    } else if narrative_support_count >= 2 {
        0.03
    } else {
        0.0
    }
}

fn support_density_bonus(
    top_support: &[ScoredCandidate<'_>],
    narrative_support_count: usize,
) -> f64 {
    if narrative_support_count < 2 {
        return 0.0;
    }

    top_support
        .iter()
        .map(|candidate| candidate.score)
        .sum::<f64>()
        .min(1.0)
        * 0.24
}

fn weighted_anchor_coverage(
    claim_anchor_tokens: &[String],
    covered_anchor_tokens: &BTreeSet<String>,
    claim_token_weights: &BTreeMap<String, f64>,
) -> f64 {
    if claim_anchor_tokens.is_empty() {
        return 0.0;
    }

    let total_weight = claim_anchor_tokens
        .iter()
        .map(|token| claim_token_weight(token, claim_token_weights))
        .sum::<f64>();

    if total_weight <= f64::EPSILON {
        return 0.0;
    }

    let matched_weight = claim_anchor_tokens
        .iter()
        .filter(|token| covered_anchor_tokens.contains(*token))
        .map(|token| claim_token_weight(token, claim_token_weights))
        .sum::<f64>();

    matched_weight / total_weight
}

fn primary_heading_supports_claim(heading: &SnapshotBlock, claim_anchor_tokens: &[String]) -> bool {
    if claim_anchor_tokens.is_empty() {
        return false;
    }

    let heading_tokens = tokenize_significant(&heading.text);
    !heading_tokens.is_empty() && token_overlap_ratio(claim_anchor_tokens, &heading_tokens) >= 0.5
}
