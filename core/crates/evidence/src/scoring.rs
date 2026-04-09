use std::collections::{BTreeMap, BTreeSet};

use touch_browser_contracts::{SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole};

use crate::{
    candidates::block_candidates,
    contradiction::contradiction_detected,
    normalization::{
        claim_mentions_version_or_release, contains_token_sequence, is_version_like_token,
        normalize_text, numeric_tokens, token_overlap_ratio, tokenize_significant, tokens_match,
    },
};

#[derive(Clone, Debug)]
pub(crate) struct ScoredCandidate<'a> {
    pub(crate) block: &'a SnapshotBlock,
    pub(crate) candidate_index: usize,
    pub(crate) text: String,
    pub(crate) score: f64,
    pub(crate) lexical_overlap: f64,
    pub(crate) contradictory: bool,
    pub(crate) exact_support: bool,
}

pub(crate) struct ScoringContext {
    pub(crate) claim_token_weights: BTreeMap<String, f64>,
}

struct CandidateScoringInput<'a> {
    normalized_claim: &'a str,
    claim_tokens: &'a [String],
    claim_qualifier_tokens: &'a [String],
    claim_numeric_tokens: &'a [String],
    scoring_context: &'a ScoringContext,
}

pub(crate) fn score_candidates<'a>(
    blocks: &'a [SnapshotBlock],
    normalized_claim: &str,
    claim_tokens: &[String],
    claim_qualifier_tokens: &[String],
    claim_numeric_tokens: &[String],
    scoring_context: &ScoringContext,
) -> Vec<ScoredCandidate<'a>> {
    let input = CandidateScoringInput {
        normalized_claim,
        claim_tokens,
        claim_qualifier_tokens,
        claim_numeric_tokens,
        scoring_context,
    };
    let mut scored = blocks
        .iter()
        .enumerate()
        .flat_map(|(index, block)| score_block_candidates(blocks, index, block, &input))
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

pub(crate) fn document_prefers_cross_lingual_matching(blocks: &[SnapshotBlock]) -> bool {
    let mut latin = 0usize;
    let mut cjk = 0usize;

    for block in blocks.iter().filter(|block| {
        matches!(
            block.role,
            SnapshotBlockRole::Content
                | SnapshotBlockRole::Supporting
                | SnapshotBlockRole::Metadata
        )
    }) {
        for character in block.text.chars() {
            if character.is_ascii_alphabetic() {
                latin += 1;
            } else if is_cjk_character(character) {
                cjk += 1;
            }
        }
    }

    latin >= 48 && latin >= cjk.saturating_mul(3).max(12)
}

pub(crate) fn build_scoring_context(
    blocks: &[SnapshotBlock],
    claim_tokens: &[String],
) -> ScoringContext {
    let block_count = blocks.len().max(1) as f64;
    let document_frequency = claim_token_document_frequency(blocks, claim_tokens);

    let claim_token_weights = claim_tokens
        .iter()
        .cloned()
        .map(|token| {
            let doc_frequency = *document_frequency.get(&token).unwrap_or(&0) as f64;
            let inverse_document_frequency =
                (((block_count - doc_frequency) + 0.5) / (doc_frequency + 0.5)).ln() + 1.0;
            (token, inverse_document_frequency.max(0.2))
        })
        .collect();

    ScoringContext {
        claim_token_weights,
    }
}

pub(crate) fn claim_token_weight(token: &str, claim_token_weights: &BTreeMap<String, f64>) -> f64 {
    claim_token_weights.get(token).copied().unwrap_or(1.0)
}

pub(crate) fn block_search_text(block: &SnapshotBlock) -> String {
    let mut parts = vec![block.text.clone()];

    for (key, value) in &block.attributes {
        match value {
            serde_json::Value::String(text) => parts.push(text.clone()),
            serde_json::Value::Bool(true) => parts.push(key.clone()),
            serde_json::Value::Number(number) => parts.push(number.to_string()),
            serde_json::Value::Array(items) => {
                parts.extend(items.iter().filter_map(search_term_from_attribute_value));
            }
            _ => {}
        }
    }

    parts.extend(block_semantic_terms(block));

    parts.join(" ")
}

pub(crate) fn weighted_token_overlap_ratio(
    claim_tokens: &[String],
    block_tokens: &[String],
    claim_token_weights: &BTreeMap<String, f64>,
) -> f64 {
    if claim_tokens.is_empty() {
        return 0.0;
    }

    let block_token_set = block_tokens.iter().cloned().collect::<BTreeSet<_>>();
    let total_weight = claim_tokens
        .iter()
        .map(|claim_token| claim_token_weight(claim_token, claim_token_weights))
        .sum::<f64>();

    if total_weight <= f64::EPSILON {
        return token_overlap_ratio(claim_tokens, block_tokens);
    }

    let matched_weight = claim_tokens
        .iter()
        .filter(|claim_token| {
            block_token_set
                .iter()
                .any(|block_token| tokens_match(claim_token, block_token))
        })
        .map(|claim_token| claim_token_weight(claim_token, claim_token_weights))
        .sum::<f64>();

    matched_weight / total_weight
}

pub(crate) fn numeric_overlap_ratio(claim_numeric_tokens: &[String], block_text: &str) -> f64 {
    if claim_numeric_tokens.is_empty() {
        return 0.0;
    }

    let block_numeric_tokens = numeric_tokens(block_text);
    let matched = claim_numeric_tokens
        .iter()
        .filter(|claim_token| block_numeric_tokens.contains(claim_token))
        .count();

    matched as f64 / claim_numeric_tokens.len() as f64
}

pub(crate) fn exact_match_bonus(normalized_claim: &str, normalized_block_text: &str) -> f64 {
    if normalized_claim.is_empty() || normalized_block_text.is_empty() {
        return 0.0;
    }

    if contains_token_sequence(normalized_block_text, normalized_claim) {
        1.0
    } else {
        let claim_tokens = crate::normalization::tokenize_all(normalized_claim);
        let block_token_set = crate::normalization::tokenize_all(normalized_block_text)
            .into_iter()
            .collect::<BTreeSet<_>>();
        if !claim_tokens.is_empty()
            && claim_tokens
                .iter()
                .all(|token| block_token_set.contains(token))
        {
            0.70
        } else {
            0.0
        }
    }
}

pub(crate) fn contextual_neighbor_indices(blocks: &[SnapshotBlock], index: usize) -> Vec<usize> {
    let mut neighbors = Vec::new();
    let region = stable_ref_region(&blocks[index]);

    for candidate_index in [index.checked_sub(1), Some(index + 1)]
        .into_iter()
        .flatten()
    {
        let Some(candidate) = blocks.get(candidate_index) else {
            continue;
        };

        if stable_ref_region(candidate) != region {
            continue;
        }

        if !is_contextual_neighbor(candidate) {
            continue;
        }

        neighbors.push(candidate_index);
    }

    neighbors
}

pub(crate) fn nearest_heading_context<'a>(
    blocks: &'a [SnapshotBlock],
    block: &SnapshotBlock,
) -> Option<&'a SnapshotBlock> {
    let block_index = blocks
        .iter()
        .position(|candidate| std::ptr::eq(candidate, block))?;

    blocks[..block_index]
        .iter()
        .rev()
        .find(|candidate| matches!(candidate.kind, SnapshotBlockKind::Heading))
}

pub(crate) fn primary_heading_context(blocks: &[SnapshotBlock]) -> Option<&SnapshotBlock> {
    blocks
        .iter()
        .find(|candidate| {
            matches!(candidate.kind, SnapshotBlockKind::Heading)
                && candidate
                    .attributes
                    .get("level")
                    .and_then(serde_json::Value::as_u64)
                    == Some(1)
        })
        .or_else(|| {
            blocks
                .iter()
                .find(|candidate| matches!(candidate.kind, SnapshotBlockKind::Heading))
        })
}

pub(crate) fn is_narrative_aggregate_block(block: &SnapshotBlock) -> bool {
    match block.kind {
        SnapshotBlockKind::Text
        | SnapshotBlockKind::List
        | SnapshotBlockKind::Table
        | SnapshotBlockKind::Metadata => block.text.chars().count() >= 12,
        SnapshotBlockKind::Heading => block.text.chars().count() >= 8,
        SnapshotBlockKind::Link => block.text.chars().count() >= 16,
        SnapshotBlockKind::Button | SnapshotBlockKind::Form | SnapshotBlockKind::Input => false,
    }
}

pub(crate) fn round_confidence(score: f64) -> f64 {
    let confidence = 0.55 + (score * 0.40);
    (confidence * 100.0).round() / 100.0
}

fn claim_token_document_frequency(
    blocks: &[SnapshotBlock],
    claim_tokens: &[String],
) -> BTreeMap<String, usize> {
    let mut document_frequency = claim_tokens
        .iter()
        .cloned()
        .map(|token| (token, 0usize))
        .collect::<BTreeMap<_, _>>();

    for block in blocks {
        let block_tokens = tokenize_significant(&block_search_text(block))
            .into_iter()
            .collect::<BTreeSet<_>>();

        for claim_token in claim_tokens {
            if block_tokens
                .iter()
                .any(|block_token| tokens_match(claim_token, block_token))
            {
                *document_frequency.entry(claim_token.clone()).or_default() += 1;
            }
        }
    }

    document_frequency
}

fn score_block_candidates<'a>(
    blocks: &'a [SnapshotBlock],
    index: usize,
    block: &'a SnapshotBlock,
    input: &CandidateScoringInput<'_>,
) -> Vec<ScoredCandidate<'a>> {
    block_candidates(block)
        .into_iter()
        .filter_map(|candidate| {
            let search_text = candidate_search_text(block, &candidate.text);
            let candidate_tokens = tokenize_significant(&search_text);
            if candidate_tokens.is_empty() {
                return None;
            }

            let contextual_text = contextual_search_text(blocks, index, &candidate.text);
            let contextual_tokens = tokenize_significant(&contextual_text);
            let lexical_overlap = weighted_token_overlap_ratio(
                input.claim_tokens,
                &candidate_tokens,
                &input.scoring_context.claim_token_weights,
            );
            let contextual_overlap = weighted_token_overlap_ratio(
                input.claim_tokens,
                &contextual_tokens,
                &input.scoring_context.claim_token_weights,
            );
            let exact_bonus =
                exact_match_bonus(input.normalized_claim, &normalize_text(&search_text));
            let numeric_overlap = numeric_overlap_ratio(input.claim_numeric_tokens, &search_text);
            let numeric_presence_bonus =
                numeric_presence_bonus(input.claim_numeric_tokens, &search_text);
            let kind_bonus = kind_score_bonus(&block.kind);
            let control_bonus = ui_control_bonus(blocks, index, input.claim_tokens, block);
            let structural_adjustment = block_structural_adjustment(block);
            let qualifier_adjustment =
                qualifier_alignment_adjustment(input.claim_qualifier_tokens, &search_text);
            let version_noise_penalty =
                version_noise_penalty(input.claim_tokens, input.claim_numeric_tokens, &search_text);
            let contextual_bonus = (contextual_overlap - lexical_overlap).max(0.0) * 0.10;
            let contradictory = contradiction_detected(input.normalized_claim, &search_text);
            let mut score = (lexical_overlap * 0.40)
                + (contextual_overlap * 0.26)
                + (exact_bonus * 0.16)
                + (numeric_overlap * 0.08)
                + numeric_presence_bonus
                + kind_bonus
                + control_bonus
                + structural_adjustment
                + qualifier_adjustment
                + version_noise_penalty
                + contextual_bonus;

            if contradictory && contextual_overlap >= 0.35 {
                score *= 0.6;
            }

            (score > 0.0).then_some(ScoredCandidate {
                block,
                candidate_index: candidate.index,
                text: search_text,
                score: score.min(1.0),
                lexical_overlap,
                contradictory,
                exact_support: exact_bonus >= 0.70,
            })
        })
        .collect()
}

fn qualifier_alignment_adjustment(claim_qualifier_tokens: &[String], support_text: &str) -> f64 {
    if claim_qualifier_tokens.is_empty() {
        return 0.0;
    }

    let claim_qualifiers = qualifier_presence_from_tokens(claim_qualifier_tokens);
    let support_qualifiers = qualifier_presence_from_text(support_text);

    if qualifier_profiles_align(claim_qualifiers, support_qualifiers) {
        return 0.14;
    }

    if qualifier_profiles_conflict(claim_qualifiers, support_qualifiers) {
        return -0.18;
    }

    0.0
}

#[derive(Clone, Copy)]
struct QualifierPresence {
    has_default: bool,
    has_maximum: bool,
    has_minimum: bool,
}

fn qualifier_presence_from_tokens(tokens: &[String]) -> QualifierPresence {
    QualifierPresence {
        has_default: tokens.iter().any(|token| token == "default"),
        has_maximum: tokens
            .iter()
            .any(|token| matches!(token.as_str(), "maximum" | "max")),
        has_minimum: tokens
            .iter()
            .any(|token| matches!(token.as_str(), "minimum" | "min")),
    }
}

fn qualifier_presence_from_text(support_text: &str) -> QualifierPresence {
    let normalized = normalize_text(support_text);
    QualifierPresence {
        has_default: normalized.contains(" by default ") || normalized.contains(" default "),
        has_maximum: normalized.contains(" maximum ")
            || normalized.contains(" max ")
            || normalized.contains(" up to ")
            || normalized.contains(" at most "),
        has_minimum: normalized.contains(" minimum ")
            || normalized.contains(" min ")
            || normalized.contains(" at least "),
    }
}

fn qualifier_profiles_align(claim: QualifierPresence, support: QualifierPresence) -> bool {
    (claim.has_default && support.has_default)
        || (claim.has_maximum && support.has_maximum)
        || (claim.has_minimum && support.has_minimum)
}

fn qualifier_profiles_conflict(claim: QualifierPresence, support: QualifierPresence) -> bool {
    (claim.has_default && (support.has_maximum || support.has_minimum))
        || (claim.has_maximum && (support.has_default || support.has_minimum))
        || (claim.has_minimum && (support.has_default || support.has_maximum))
}

pub(crate) fn semantic_similarity_bonus(semantic_similarity: f64, lexical_overlap: f64) -> f64 {
    if lexical_overlap <= 0.08 || semantic_similarity <= 0.68 {
        return 0.0;
    }

    let semantic_signal = ((semantic_similarity - 0.68) / 0.22).clamp(0.0, 1.0);
    let lexical_gap = ((0.55 - lexical_overlap) / 0.55).clamp(0.0, 1.0);

    semantic_signal * lexical_gap * 0.10
}

fn contextual_search_text(blocks: &[SnapshotBlock], index: usize, primary_text: &str) -> String {
    let block = &blocks[index];
    if !allows_context_expansion(block) {
        return candidate_search_text(block, primary_text);
    }

    let mut seen_blocks = BTreeSet::new();
    let mut parts = Vec::new();

    if let Some(heading) = nearest_heading_context(blocks, block) {
        if seen_blocks.insert(heading.id.clone()) {
            parts.push(block_search_text(heading));
        }
    }

    if seen_blocks.insert(block.id.clone()) {
        parts.push(candidate_search_text(block, primary_text));
    }

    for neighbor_index in contextual_neighbor_indices(blocks, index) {
        let neighbor = &blocks[neighbor_index];
        if seen_blocks.insert(neighbor.id.clone()) {
            parts.push(block_search_text(neighbor));
        }
    }

    parts.join(" ")
}

fn candidate_search_text(block: &SnapshotBlock, candidate_text: &str) -> String {
    let mut parts = vec![candidate_text.trim().to_string()];

    if let Some(header_context) = block_header_context(block, candidate_text) {
        parts.push(header_context);
    }

    parts.extend(block_search_terms_without_text(block));
    parts.join(" ")
}

fn block_search_terms_without_text(block: &SnapshotBlock) -> Vec<String> {
    let mut parts = Vec::new();

    for (key, value) in &block.attributes {
        match value {
            serde_json::Value::String(text) => parts.push(text.clone()),
            serde_json::Value::Bool(true) => parts.push(key.clone()),
            serde_json::Value::Number(number) => parts.push(number.to_string()),
            serde_json::Value::Array(items) => {
                parts.extend(items.iter().filter_map(search_term_from_attribute_value));
            }
            _ => {}
        }
    }

    parts.extend(block_semantic_terms(block));
    parts
}

fn block_header_context(block: &SnapshotBlock, candidate_text: &str) -> Option<String> {
    match block.kind {
        SnapshotBlockKind::Table | SnapshotBlockKind::List => {
            let header = block
                .text
                .lines()
                .next()
                .map(str::trim)
                .filter(|header| !header.is_empty() && *header != candidate_text.trim())?;
            Some(header.to_string())
        }
        _ => None,
    }
}

fn allows_context_expansion(block: &SnapshotBlock) -> bool {
    matches!(
        block.kind,
        SnapshotBlockKind::Heading
            | SnapshotBlockKind::Text
            | SnapshotBlockKind::List
            | SnapshotBlockKind::Table
            | SnapshotBlockKind::Metadata
    ) && matches!(
        block.role,
        SnapshotBlockRole::Content
            | SnapshotBlockRole::Supporting
            | SnapshotBlockRole::Metadata
            | SnapshotBlockRole::TableCell
    )
}

fn stable_ref_region(block: &SnapshotBlock) -> &str {
    block.stable_ref.split(':').next().unwrap_or_default()
}

fn is_contextual_neighbor(block: &SnapshotBlock) -> bool {
    match block.kind {
        SnapshotBlockKind::Heading => false,
        SnapshotBlockKind::Text | SnapshotBlockKind::List | SnapshotBlockKind::Table => {
            block.text.trim().chars().count() >= 16
        }
        SnapshotBlockKind::Metadata => block.text.trim().chars().count() >= 24,
        SnapshotBlockKind::Link => {
            matches!(
                block.role,
                SnapshotBlockRole::Content | SnapshotBlockRole::Supporting
            ) && block.text.trim().chars().count() >= 20
        }
        SnapshotBlockKind::Button | SnapshotBlockKind::Form | SnapshotBlockKind::Input => false,
    }
}

fn block_structural_adjustment(block: &SnapshotBlock) -> f64 {
    let stable_ref = block.stable_ref.to_ascii_lowercase();
    let has_available_options = matches!(
        block.attributes.get("selectionSemantic"),
        Some(serde_json::Value::String(value)) if value == "available-options"
    );
    let role_adjustment = match block.role {
        SnapshotBlockRole::Content => 0.04,
        SnapshotBlockRole::Supporting => 0.01,
        SnapshotBlockRole::Metadata => -0.04,
        SnapshotBlockRole::PrimaryNav | SnapshotBlockRole::SecondaryNav => -0.24,
        SnapshotBlockRole::Cta => -0.16,
        SnapshotBlockRole::FormControl if has_available_options => -0.05,
        SnapshotBlockRole::FormControl => -0.30,
        SnapshotBlockRole::TableCell => 0.05,
    };

    let region_adjustment = if stable_ref.starts_with("rmain:") {
        0.05
    } else if stable_ref.starts_with("rnav:") {
        -0.18
    } else if stable_ref.starts_with("rfooter:") {
        -0.16
    } else {
        0.0
    };

    role_adjustment + region_adjustment
}

fn search_term_from_attribute_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(text) => Some(text.clone()),
        serde_json::Value::Bool(true) => Some("true".to_string()),
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

fn block_semantic_terms(block: &SnapshotBlock) -> Vec<String> {
    let mut parts = Vec::new();
    let normalized_text = normalize_text(&block.text);
    let selection_semantic = block
        .attributes
        .get("selectionSemantic")
        .and_then(serde_json::Value::as_str);

    match block.kind {
        SnapshotBlockKind::List => {
            parts.push("list".to_string());
            parts.push("items".to_string());
        }
        SnapshotBlockKind::Link => {
            parts.push("link".to_string());
            if let Some(href) = block
                .attributes
                .get("href")
                .and_then(serde_json::Value::as_str)
            {
                if href.starts_with("http://") || href.starts_with("https://") {
                    parts.push("external".to_string());
                    parts.push("external-link".to_string());
                }
            }
        }
        SnapshotBlockKind::Button => {
            parts.push("button".to_string());
        }
        SnapshotBlockKind::Form => {
            parts.push("form".to_string());
            parts.push("field".to_string());
            parts.push("fields".to_string());
            parts.push("input".to_string());
        }
        SnapshotBlockKind::Input => {
            parts.push("input".to_string());
            parts.push("field".to_string());
        }
        _ => {}
    }

    if selection_semantic == Some("available-options") {
        parts.push("option".to_string());
        parts.push("options".to_string());
        parts.push("available".to_string());
        parts.push("availability".to_string());
        parts.push("supported".to_string());
    }

    if normalized_text.contains("submit") {
        parts.push("submission".to_string());
    }

    if normalized_text.contains("execute") {
        parts.push("execution".to_string());
    }

    parts
}

fn numeric_presence_bonus(claim_numeric_tokens: &[String], block_text: &str) -> f64 {
    if claim_numeric_tokens.is_empty() {
        return 0.0;
    }

    if numeric_tokens(block_text).is_empty() {
        0.0
    } else {
        0.06
    }
}

fn version_noise_penalty(
    claim_tokens: &[String],
    claim_numeric_tokens: &[String],
    contextual_text: &str,
) -> f64 {
    if !claim_numeric_tokens.is_empty() || claim_mentions_version_or_release(claim_tokens) {
        return 0.0;
    }

    let normalized_context = normalize_text(contextual_text);
    let context_tokens = normalized_context
        .split_whitespace()
        .collect::<BTreeSet<_>>();

    let mentions_version_marker = context_tokens
        .iter()
        .any(|token| is_version_like_token(token));
    let mentions_release_flow = context_tokens
        .iter()
        .any(|token| RELEASE_NOISE_TOKENS.contains(token));

    if mentions_version_marker || mentions_release_flow {
        -0.08
    } else {
        0.0
    }
}

fn kind_score_bonus(kind: &SnapshotBlockKind) -> f64 {
    match kind {
        SnapshotBlockKind::Table => 0.12,
        SnapshotBlockKind::List => 0.08,
        SnapshotBlockKind::Text => 0.06,
        SnapshotBlockKind::Heading => 0.03,
        SnapshotBlockKind::Link => 0.03,
        SnapshotBlockKind::Metadata => 0.02,
        SnapshotBlockKind::Form => 0.08,
        SnapshotBlockKind::Input => 0.06,
        SnapshotBlockKind::Button => 0.01,
    }
}

fn is_cjk_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3040..=0x30ff
            | 0x3400..=0x4dbf
            | 0x4e00..=0x9fff
            | 0xac00..=0xd7af
            | 0xf900..=0xfaff
    )
}

fn ui_control_bonus(
    blocks: &[SnapshotBlock],
    index: usize,
    claim_tokens: &[String],
    block: &SnapshotBlock,
) -> f64 {
    let claim_token_set = claim_tokens
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let mentions_button = claim_token_set.contains("button") || claim_token_set.contains("sign");
    let mentions_auth_control = claim_token_set.iter().any(|token| {
        matches!(
            *token,
            "sign"
                | "login"
                | "submit"
                | "email"
                | "password"
                | "verification"
                | "code"
                | "credential"
        )
    });
    let mentions_field = claim_token_set.contains("field")
        || claim_token_set.contains("input")
        || claim_token_set.contains("form")
        || claim_token_set.contains("email")
        || claim_token_set.contains("password")
        || claim_token_set.contains("verification")
        || claim_token_set.contains("code")
        || claim_token_set.contains("credential");
    let mentions_availability = claim_token_set.contains("available")
        || claim_token_set.contains("availability")
        || claim_token_set.contains("support")
        || claim_token_set.contains("supported");
    let mentions_platform = claim_token_set.iter().any(|token| {
        matches!(
            *token,
            "platform" | "operating" | "system" | "os" | "macos" | "windows" | "linux"
        )
    });
    let has_available_options = matches!(
        block.attributes.get("selectionSemantic"),
        Some(serde_json::Value::String(value)) if value == "available-options"
    );

    match block.kind {
        SnapshotBlockKind::Input
            if has_available_options && mentions_availability && mentions_platform =>
        {
            0.24
        }
        SnapshotBlockKind::List
            if has_available_options && mentions_availability && mentions_platform =>
        {
            0.18
        }
        SnapshotBlockKind::Button if mentions_button && mentions_auth_control => 0.22,
        SnapshotBlockKind::Button
            if mentions_button && button_claim_has_context(blocks, index, claim_tokens) =>
        {
            0.18
        }
        SnapshotBlockKind::Input if mentions_field => 0.20,
        SnapshotBlockKind::Form if mentions_field => 0.16,
        _ => 0.0,
    }
}

fn button_claim_has_context(
    blocks: &[SnapshotBlock],
    index: usize,
    claim_tokens: &[String],
) -> bool {
    let contextual_tokens = contextual_neighbor_indices(blocks, index)
        .into_iter()
        .flat_map(|neighbor_index| {
            tokenize_significant(&block_search_text(&blocks[neighbor_index]))
        })
        .collect::<BTreeSet<_>>();
    let meaningful_claim_tokens = claim_tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.as_str(),
                "button" | "field" | "form" | "input" | "contain" | "contains" | "includ"
            ) && !token.chars().all(|character| character.is_ascii_digit())
        })
        .collect::<Vec<_>>();

    meaningful_claim_tokens.iter().any(|token| {
        contextual_tokens
            .iter()
            .any(|candidate| tokens_match(token, candidate))
    })
}

const RELEASE_NOISE_TOKENS: &[&str] = &[
    "upgrade",
    "upgrading",
    "changelog",
    "release",
    "releases",
    "remix",
    "migration",
    "migrat",
];
