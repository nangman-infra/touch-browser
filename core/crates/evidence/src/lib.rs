use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

use ferrous_opencc::{config::BuiltinConfig, OpenCC};
use thiserror::Error;
use touch_browser_contracts::{
    EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceGuardFailure,
    EvidenceGuardKind, EvidenceReport, EvidenceSource, SnapshotBlock, SnapshotBlockKind,
    SnapshotDocument, SourceRisk, UnsupportedClaimReason, CONTRACT_VERSION,
};

pub fn crate_status() -> &'static str {
    "evidence ready"
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimRequest {
    pub claim_id: String,
    pub statement: String,
}

impl ClaimRequest {
    pub fn new(claim_id: impl Into<String>, statement: impl Into<String>) -> Self {
        Self {
            claim_id: claim_id.into(),
            statement: statement.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EvidenceInput {
    pub snapshot: SnapshotDocument,
    pub claims: Vec<ClaimRequest>,
    pub generated_at: String,
    pub source_risk: SourceRisk,
    pub source_label: Option<String>,
}

impl EvidenceInput {
    pub fn new(
        snapshot: SnapshotDocument,
        claims: Vec<ClaimRequest>,
        generated_at: impl Into<String>,
        source_risk: SourceRisk,
        source_label: Option<String>,
    ) -> Self {
        Self {
            snapshot,
            claims,
            generated_at: generated_at.into(),
            source_risk,
            source_label,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct EvidenceExtractor;

impl EvidenceExtractor {
    pub fn extract(&self, input: &EvidenceInput) -> Result<EvidenceReport, EvidenceError> {
        if input.claims.is_empty() {
            return Err(EvidenceError::NoClaims);
        }

        let mut claim_outcomes = Vec::new();

        for claim in &input.claims {
            let resolution = analyze_claim(claim, &input.snapshot.blocks);
            let citation =
                (resolution.verdict == EvidenceClaimVerdict::EvidenceSupported).then(|| {
                    EvidenceCitation {
                        url: input.snapshot.source.source_url.clone(),
                        retrieved_at: input.generated_at.clone(),
                        source_type: input.snapshot.source.source_type.clone(),
                        source_risk: input.source_risk.clone(),
                        source_label: input
                            .source_label
                            .clone()
                            .or_else(|| input.snapshot.source.title.clone()),
                    }
                });

            claim_outcomes.push(EvidenceClaimOutcome {
                version: CONTRACT_VERSION.to_string(),
                claim_id: claim.claim_id.clone(),
                statement: claim.statement.clone(),
                verdict: resolution.verdict,
                support: resolution
                    .support
                    .iter()
                    .map(|candidate| candidate.block.id.clone())
                    .collect(),
                support_score: resolution.confidence,
                citation,
                reason: resolution.reason,
                checked_block_refs: resolution.checked_refs,
                guard_failures: resolution.guard_failures,
                next_action_hint: resolution.next_action_hint,
                verification_verdict: None,
            });
        }

        let mut report = EvidenceReport {
            version: CONTRACT_VERSION.to_string(),
            generated_at: input.generated_at.clone(),
            source: EvidenceSource {
                source_url: input.snapshot.source.source_url.clone(),
                source_type: input.snapshot.source.source_type.clone(),
                source_risk: input.source_risk.clone(),
                source_label: input
                    .source_label
                    .clone()
                    .or_else(|| input.snapshot.source.title.clone()),
            },
            supported_claims: Vec::new(),
            contradicted_claims: Vec::new(),
            unsupported_claims: Vec::new(),
            needs_more_browsing_claims: Vec::new(),
            claim_outcomes,
            verification: None,
        };
        report.rebuild_claim_buckets();
        Ok(report)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceError {
    #[error("evidence extractor requires at least one claim")]
    NoClaims,
}

#[derive(Debug)]
struct ClaimResolution<'a> {
    verdict: EvidenceClaimVerdict,
    support: Vec<ScoredCandidate<'a>>,
    confidence: Option<f64>,
    reason: Option<UnsupportedClaimReason>,
    checked_refs: Vec<String>,
    guard_failures: Vec<EvidenceGuardFailure>,
    next_action_hint: Option<String>,
}

#[derive(Clone, Debug)]
struct ScoredCandidate<'a> {
    block: &'a SnapshotBlock,
    score: f64,
    contradictory: bool,
}

struct ClaimAnalysisInput {
    claim_tokens: Vec<String>,
    claim_numeric_tokens: Vec<String>,
    claim_anchor_tokens: Vec<String>,
    claim_qualifier_tokens: Vec<String>,
    normalized_claim: String,
}

struct ScoringContext {
    claim_token_weights: BTreeMap<String, f64>,
}

fn analyze_claim<'a>(claim: &ClaimRequest, blocks: &'a [SnapshotBlock]) -> ClaimResolution<'a> {
    let analysis = build_claim_analysis_input(claim);
    let scoring_context = build_scoring_context(blocks, &analysis.claim_tokens);
    let scored = score_candidates(blocks, &analysis, &scoring_context);
    let checked_refs = checked_refs(&scored);
    let contradictory_support = contradictory_support(&scored);

    if let Some(resolution) =
        contradiction_resolution(claim, &contradictory_support, blocks, &analysis)
    {
        return resolution;
    }

    let contradictory_exists = scored.iter().any(|candidate| candidate.contradictory);
    let non_contradictory = non_contradictory_candidates(scored);

    let Some(best_candidate) = non_contradictory.first() else {
        return no_candidate_resolution(contradictory_exists, checked_refs);
    };
    let best_score = best_candidate.score;
    let top_support = top_support_candidates(non_contradictory);

    if top_support.is_empty() {
        return no_top_support_resolution(contradictory_exists, checked_refs);
    }

    let assessment = assess_support_guards(
        claim,
        &top_support,
        blocks,
        &analysis.claim_anchor_tokens,
        &analysis.claim_qualifier_tokens,
    );
    let effective_score = effective_support_score(
        claim,
        &analysis,
        &top_support,
        blocks,
        &scoring_context,
        best_score,
    );
    let support_threshold = support_acceptance_threshold(&top_support, &assessment);

    if let Some(resolution) = guarded_resolution(
        claim,
        &analysis,
        &top_support,
        &checked_refs,
        &assessment,
        effective_score,
        support_threshold,
    ) {
        return resolution;
    }

    supported_resolution(best_score, top_support, checked_refs)
}

fn build_claim_analysis_input(claim: &ClaimRequest) -> ClaimAnalysisInput {
    let claim_tokens = tokenize_significant(&claim.statement);
    ClaimAnalysisInput {
        claim_numeric_tokens: numeric_tokens(&claim.statement),
        claim_anchor_tokens: anchor_tokens(&claim_tokens),
        claim_qualifier_tokens: qualifier_tokens(&claim.statement),
        normalized_claim: normalize_text(&claim.statement),
        claim_tokens,
    }
}

fn score_candidates<'a>(
    blocks: &'a [SnapshotBlock],
    analysis: &ClaimAnalysisInput,
    scoring_context: &ScoringContext,
) -> Vec<ScoredCandidate<'a>> {
    let mut scored = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            score_block(
                blocks,
                index,
                block,
                &analysis.normalized_claim,
                &analysis.claim_tokens,
                &analysis.claim_numeric_tokens,
                scoring_context,
            )
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored
}

fn build_scoring_context(blocks: &[SnapshotBlock], claim_tokens: &[String]) -> ScoringContext {
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

fn checked_refs(scored: &[ScoredCandidate<'_>]) -> Vec<String> {
    scored
        .iter()
        .take(3)
        .map(|candidate| candidate.block.stable_ref.clone())
        .collect()
}

fn contradictory_support<'a>(scored: &[ScoredCandidate<'a>]) -> Vec<ScoredCandidate<'a>> {
    scored
        .iter()
        .filter(|candidate| candidate.contradictory && candidate.score >= 0.05)
        .take(3)
        .cloned()
        .collect()
}

fn contradiction_resolution<'a>(
    claim: &ClaimRequest,
    contradictory_support: &[ScoredCandidate<'a>],
    blocks: &'a [SnapshotBlock],
    analysis: &ClaimAnalysisInput,
) -> Option<ClaimResolution<'a>> {
    if contradictory_support.is_empty() {
        return None;
    }

    let contradiction_checked_refs = contradictory_support
        .iter()
        .map(|candidate| candidate.block.stable_ref.clone())
        .collect::<Vec<_>>();
    let assessment = assess_support_guards(
        claim,
        contradictory_support,
        blocks,
        &analysis.claim_anchor_tokens,
        &analysis.claim_qualifier_tokens,
    );
    let reason = assessment.contradiction_reason.clone()?;

    Some(ClaimResolution {
        verdict: EvidenceClaimVerdict::Contradicted,
        support: contradictory_support.to_vec(),
        confidence: None,
        reason: Some(reason),
        checked_refs: contradiction_checked_refs,
        guard_failures: assessment.guard_failures,
        next_action_hint: None,
    })
}

fn non_contradictory_candidates<'a>(scored: Vec<ScoredCandidate<'a>>) -> Vec<ScoredCandidate<'a>> {
    scored
        .into_iter()
        .filter(|candidate| !candidate.contradictory)
        .collect()
}

fn no_candidate_resolution<'a>(
    contradictory_exists: bool,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    if contradictory_exists {
        ClaimResolution {
            verdict: EvidenceClaimVerdict::Contradicted,
            support: Vec::new(),
            confidence: None,
            reason: Some(UnsupportedClaimReason::ContradictoryEvidence),
            checked_refs,
            guard_failures: vec![EvidenceGuardFailure {
                kind: EvidenceGuardKind::Negation,
                detail: "Observed support blocks contradict the claim polarity.".to_string(),
            }],
            next_action_hint: None,
        }
    } else {
        ClaimResolution {
            verdict: EvidenceClaimVerdict::InsufficientEvidence,
            support: Vec::new(),
            confidence: None,
            reason: Some(UnsupportedClaimReason::NoSupportingBlock),
            checked_refs,
            guard_failures: Vec::new(),
            next_action_hint: None,
        }
    }
}

fn top_support_candidates<'a>(
    non_contradictory: Vec<ScoredCandidate<'a>>,
) -> Vec<ScoredCandidate<'a>> {
    non_contradictory
        .into_iter()
        .filter(|candidate| candidate.score >= 0.22)
        .take(3)
        .collect()
}

fn no_top_support_resolution<'a>(
    contradictory_exists: bool,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    let reason = if contradictory_exists {
        UnsupportedClaimReason::ContradictoryEvidence
    } else {
        UnsupportedClaimReason::InsufficientConfidence
    };

    ClaimResolution {
        verdict: if contradictory_exists {
            EvidenceClaimVerdict::Contradicted
        } else {
            EvidenceClaimVerdict::InsufficientEvidence
        },
        support: Vec::new(),
        confidence: None,
        reason: Some(reason),
        checked_refs,
        guard_failures: Vec::new(),
        next_action_hint: None,
    }
}

fn effective_support_score(
    claim: &ClaimRequest,
    analysis: &ClaimAnalysisInput,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    scoring_context: &ScoringContext,
    best_score: f64,
) -> f64 {
    let aggregated_score = aggregate_support_score(
        claim,
        &analysis.normalized_claim,
        &analysis.claim_tokens,
        &analysis.claim_numeric_tokens,
        top_support,
        blocks,
        scoring_context,
    );
    best_score.max(aggregated_score)
}

fn guarded_resolution<'a>(
    claim: &ClaimRequest,
    analysis: &ClaimAnalysisInput,
    top_support: &[ScoredCandidate<'a>],
    checked_refs: &[String],
    assessment: &GuardAssessment,
    effective_score: f64,
    support_threshold: f64,
) -> Option<ClaimResolution<'a>> {
    if let Some(reason) = assessment.contradiction_reason.clone() {
        return Some(ClaimResolution {
            verdict: EvidenceClaimVerdict::Contradicted,
            support: top_support.to_vec(),
            confidence: None,
            reason: Some(reason),
            checked_refs: checked_refs.to_vec(),
            guard_failures: assessment.guard_failures.clone(),
            next_action_hint: None,
        });
    }

    if effective_score >= support_threshold && assessment.guard_failures.is_empty() {
        if button_claim_requires_more_browsing(&analysis.claim_tokens, top_support) {
            return Some(ClaimResolution {
                verdict: EvidenceClaimVerdict::NeedsMoreBrowsing,
                support: top_support.to_vec(),
                confidence: None,
                reason: Some(UnsupportedClaimReason::NeedsMoreBrowsing),
                checked_refs: checked_refs.to_vec(),
                guard_failures: Vec::new(),
                next_action_hint: Some(
                    "Browse a more specific source page before answering.".to_string(),
                ),
            });
        }
        return None;
    }

    let verdict = if should_keep_browsing(effective_score, assessment, claim) {
        EvidenceClaimVerdict::NeedsMoreBrowsing
    } else {
        EvidenceClaimVerdict::InsufficientEvidence
    };

    Some(ClaimResolution {
        verdict: verdict.clone(),
        support: top_support.to_vec(),
        confidence: None,
        reason: Some(if verdict == EvidenceClaimVerdict::NeedsMoreBrowsing {
            UnsupportedClaimReason::NeedsMoreBrowsing
        } else {
            UnsupportedClaimReason::InsufficientConfidence
        }),
        checked_refs: checked_refs.to_vec(),
        guard_failures: assessment.guard_failures.clone(),
        next_action_hint: assessment.next_action_hint.clone(),
    })
}

fn supported_resolution<'a>(
    best_score: f64,
    top_support: Vec<ScoredCandidate<'a>>,
    checked_refs: Vec<String>,
) -> ClaimResolution<'a> {
    let confidence = round_confidence(
        best_score.max(
            top_support
                .iter()
                .map(|candidate| candidate.score)
                .fold(0.0, f64::max),
        ),
    );

    ClaimResolution {
        verdict: EvidenceClaimVerdict::EvidenceSupported,
        support: top_support,
        confidence: Some(confidence),
        reason: None,
        checked_refs,
        guard_failures: Vec::new(),
        next_action_hint: None,
    }
}

fn support_acceptance_threshold(
    top_support: &[ScoredCandidate<'_>],
    assessment: &GuardAssessment,
) -> f64 {
    if !assessment.guard_failures.is_empty() || top_support.len() < 2 {
        return 0.52;
    }

    let narrative_support_count = top_support
        .iter()
        .filter(|candidate| is_narrative_aggregate_block(candidate.block))
        .count();
    if narrative_support_count >= 2 {
        0.46
    } else {
        0.52
    }
}

fn button_claim_requires_more_browsing(
    claim_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
) -> bool {
    let claim_token_set = claim_tokens
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    if !claim_token_set.contains("button") {
        return false;
    }

    if claim_token_set.iter().any(|token| {
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
    }) {
        return false;
    }

    if top_support.iter().any(|candidate| {
        matches!(
            candidate.block.kind,
            SnapshotBlockKind::Form | SnapshotBlockKind::Input
        )
    }) {
        return false;
    }

    let meaningful_claim_tokens = claim_tokens
        .iter()
        .filter(|token| {
            !matches!(
                token.as_str(),
                "button" | "contain" | "contains" | "includ" | "page"
            ) && !token.chars().all(|character| character.is_ascii_digit())
        })
        .collect::<Vec<_>>();

    if meaningful_claim_tokens.is_empty() {
        return true;
    }

    let corroborating_non_button = top_support.iter().any(|candidate| {
        !matches!(candidate.block.kind, SnapshotBlockKind::Button)
            && meaningful_claim_tokens.iter().any(|claim_token| {
                tokenize_significant(&block_search_text(candidate.block))
                    .iter()
                    .any(|token| tokens_match(claim_token, token))
            })
    });

    !corroborating_non_button
}

#[derive(Debug, Default)]
struct GuardAssessment {
    contradiction_reason: Option<UnsupportedClaimReason>,
    guard_failures: Vec<EvidenceGuardFailure>,
    next_action_hint: Option<String>,
}

struct GuardCheck {
    contradiction_reason: Option<UnsupportedClaimReason>,
    failure: Option<EvidenceGuardFailure>,
}

fn score_block<'a>(
    blocks: &'a [SnapshotBlock],
    index: usize,
    block: &'a SnapshotBlock,
    normalized_claim: &str,
    claim_tokens: &[String],
    claim_numeric_tokens: &[String],
    scoring_context: &ScoringContext,
) -> Option<ScoredCandidate<'a>> {
    let search_text = block_search_text(block);
    let block_tokens = tokenize_significant(&search_text);
    if block_tokens.is_empty() {
        return None;
    }

    let contextual_text = contextual_search_text(blocks, index);
    let contextual_tokens = tokenize_significant(&contextual_text);
    let lexical_overlap = weighted_token_overlap_ratio(
        claim_tokens,
        &block_tokens,
        &scoring_context.claim_token_weights,
    );
    let contextual_overlap = weighted_token_overlap_ratio(
        claim_tokens,
        &contextual_tokens,
        &scoring_context.claim_token_weights,
    );
    let exact_bonus = exact_match_bonus(normalized_claim, &normalize_text(&contextual_text));
    let numeric_overlap = numeric_overlap_ratio(claim_numeric_tokens, &contextual_text);
    let numeric_presence_bonus = numeric_presence_bonus(claim_numeric_tokens, &contextual_text);
    let kind_bonus = kind_score_bonus(&block.kind);
    let control_bonus = ui_control_bonus(blocks, index, claim_tokens, block);
    let structural_adjustment = block_structural_adjustment(block);
    let version_noise_penalty =
        version_noise_penalty(claim_tokens, claim_numeric_tokens, &contextual_text);
    let contextual_bonus = (contextual_overlap - lexical_overlap).max(0.0) * 0.10;
    let contradictory = contradiction_detected(normalized_claim, &contextual_text)
        || contradiction_detected(normalized_claim, &block.text);
    let mut score = (lexical_overlap * 0.40)
        + (contextual_overlap * 0.26)
        + (exact_bonus * 0.16)
        + (numeric_overlap * 0.08)
        + numeric_presence_bonus
        + kind_bonus
        + control_bonus
        + structural_adjustment
        + version_noise_penalty
        + contextual_bonus;

    if contradictory && contextual_overlap >= 0.35 {
        score *= 0.6;
    }

    (score > 0.0).then_some(ScoredCandidate {
        block,
        score: score.min(1.0),
        contradictory,
    })
}

fn contextual_search_text(blocks: &[SnapshotBlock], index: usize) -> String {
    let block = &blocks[index];
    if !allows_context_expansion(block) {
        return block_search_text(block);
    }

    let mut seen_blocks = BTreeSet::new();
    let mut parts = Vec::new();

    if let Some(heading) = nearest_heading_context(blocks, block) {
        if seen_blocks.insert(heading.id.clone()) {
            parts.push(block_search_text(heading));
        }
    }

    if seen_blocks.insert(block.id.clone()) {
        parts.push(block_search_text(block));
    }

    for neighbor_index in contextual_neighbor_indices(blocks, index) {
        let neighbor = &blocks[neighbor_index];
        if seen_blocks.insert(neighbor.id.clone()) {
            parts.push(block_search_text(neighbor));
        }
    }

    parts.join(" ")
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
        touch_browser_contracts::SnapshotBlockRole::Content
            | touch_browser_contracts::SnapshotBlockRole::Supporting
            | touch_browser_contracts::SnapshotBlockRole::Metadata
            | touch_browser_contracts::SnapshotBlockRole::TableCell
    )
}

fn contextual_neighbor_indices(blocks: &[SnapshotBlock], index: usize) -> Vec<usize> {
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
                touch_browser_contracts::SnapshotBlockRole::Content
                    | touch_browser_contracts::SnapshotBlockRole::Supporting
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
        touch_browser_contracts::SnapshotBlockRole::Content => 0.04,
        touch_browser_contracts::SnapshotBlockRole::Supporting => 0.01,
        touch_browser_contracts::SnapshotBlockRole::Metadata => -0.04,
        touch_browser_contracts::SnapshotBlockRole::PrimaryNav
        | touch_browser_contracts::SnapshotBlockRole::SecondaryNav => -0.24,
        touch_browser_contracts::SnapshotBlockRole::Cta => -0.16,
        touch_browser_contracts::SnapshotBlockRole::FormControl if has_available_options => -0.05,
        touch_browser_contracts::SnapshotBlockRole::FormControl => -0.30,
        touch_browser_contracts::SnapshotBlockRole::TableCell => 0.05,
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

fn contradiction_detected(normalized_claim: &str, block_text: &str) -> bool {
    let normalized_block = normalize_text(block_text);

    if normalized_claim.is_empty() || normalized_block.is_empty() {
        return false;
    }

    CONTRADICTION_PATTERNS.iter().any(|pattern| {
        let claim_positive = contains_phrase(normalized_claim, pattern.positive);
        let claim_negative = contains_phrase(normalized_claim, pattern.negative);
        let block_positive = contains_phrase(&normalized_block, pattern.positive);
        let block_negative = contains_phrase(&normalized_block, pattern.negative);

        (claim_positive && block_negative) || (claim_negative && block_positive)
    })
}

fn contains_phrase(text: &str, phrase: &str) -> bool {
    contains_token_sequence(text, phrase)
}

fn block_search_text(block: &SnapshotBlock) -> String {
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

fn token_overlap_ratio(claim_tokens: &[String], block_tokens: &[String]) -> f64 {
    if claim_tokens.is_empty() {
        return 0.0;
    }

    let block_token_set = block_tokens.iter().cloned().collect::<BTreeSet<_>>();
    let matched = claim_tokens
        .iter()
        .filter(|claim_token| {
            block_token_set
                .iter()
                .any(|block_token| tokens_match(claim_token, block_token))
        })
        .count();

    matched as f64 / claim_tokens.len() as f64
}

fn weighted_token_overlap_ratio(
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

fn claim_token_weight(token: &str, claim_token_weights: &BTreeMap<String, f64>) -> f64 {
    claim_token_weights.get(token).copied().unwrap_or(1.0)
}

fn numeric_overlap_ratio(claim_numeric_tokens: &[String], block_text: &str) -> f64 {
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

fn claim_mentions_version_or_release(claim_tokens: &[String]) -> bool {
    claim_tokens
        .iter()
        .any(|token| RELEASE_NOISE_TOKENS.contains(&token.as_str()) || is_version_like_token(token))
}

fn is_version_like_token(token: &str) -> bool {
    if let Some(rest) = token.strip_prefix('v') {
        return !rest.is_empty()
            && rest
                .chars()
                .all(|character| character.is_ascii_digit() || character == '.');
    }

    token.chars().filter(|character| *character == '.').count() >= 1
        && token
            .chars()
            .all(|character| character.is_ascii_digit() || character == '.')
}

fn exact_match_bonus(normalized_claim: &str, normalized_block_text: &str) -> f64 {
    if normalized_claim.is_empty() || normalized_block_text.is_empty() {
        return 0.0;
    }

    if contains_token_sequence(normalized_block_text, normalized_claim) {
        1.0
    } else {
        let claim_tokens = tokenize_all(normalized_claim);
        let block_token_set = tokenize_all(normalized_block_text)
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

fn contains_token_sequence(text: &str, phrase: &str) -> bool {
    let text_tokens = tokenize_all(text);
    let phrase_tokens = tokenize_all(phrase);
    if phrase_tokens.is_empty() || text_tokens.len() < phrase_tokens.len() {
        return false;
    }

    text_tokens
        .windows(phrase_tokens.len())
        .any(|window| window == phrase_tokens.as_slice())
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

fn round_confidence(score: f64) -> f64 {
    let confidence = 0.55 + (score * 0.40);
    (confidence * 100.0).round() / 100.0
}

fn tokenize_significant(text: &str) -> Vec<String> {
    split_normalized_tokens(&normalize_text(text))
        .into_iter()
        .flat_map(|token| expand_semantic_tokens(&token, true))
        .filter(|token| is_significant_token(token))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn numeric_tokens(text: &str) -> Vec<String> {
    normalize_text(text)
        .split_whitespace()
        .map(|token| token.replace(',', ""))
        .filter(|token| {
            !token.is_empty() && token.chars().all(|character| character.is_ascii_digit())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn tokenize_all(text: &str) -> Vec<String> {
    split_normalized_tokens(&normalize_text(text))
        .into_iter()
        .flat_map(|token| expand_semantic_tokens(&token, false))
        .filter(|token| !token.is_empty())
        .collect()
}

fn normalize_text(text: &str) -> String {
    let normalized_source = normalize_chinese_variants(text);
    let mut normalized = String::with_capacity(text.len());

    for character in normalized_source
        .chars()
        .flat_map(|character| character.to_lowercase())
    {
        if character.is_alphanumeric() || is_cjk_character(character) {
            normalized.push(character);
        } else {
            normalized.push(' ');
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_chinese_variants(text: &str) -> String {
    if !should_fold_chinese_variants(text) {
        return text.to_string();
    }

    chinese_t2s_converter()
        .map(|converter| converter.convert(text))
        .unwrap_or_else(|| text.to_string())
}

fn chinese_t2s_converter() -> Option<&'static OpenCC> {
    static CONVERTER: OnceLock<Option<OpenCC>> = OnceLock::new();

    CONVERTER
        .get_or_init(|| OpenCC::from_config(BuiltinConfig::T2s).ok())
        .as_ref()
}

fn should_fold_chinese_variants(text: &str) -> bool {
    text.chars().any(is_han_character)
        && !text.chars().any(is_japanese_kana_character)
        && !text.chars().any(is_hangul_character)
}

fn stem_token(token: &str) -> String {
    if !token.is_ascii() {
        return token.to_string();
    }

    let mut stemmed = token.to_string();

    for suffix in ["ing", "ed", "ly", "es", "s"] {
        if stemmed.len() > suffix.len() + 2 && stemmed.ends_with(suffix) {
            stemmed.truncate(stemmed.len() - suffix.len());
            break;
        }
    }

    stemmed
}

fn is_significant_token(token: &str) -> bool {
    if token.chars().all(|character| character.is_ascii_digit()) {
        return true;
    }

    if contains_cjk(token) {
        return token.chars().count() >= 2;
    }

    token.len() >= 3 && !STOP_WORDS.contains(&token)
}

fn tokens_match(left: &str, right: &str) -> bool {
    if !left.is_ascii() || !right.is_ascii() {
        return left == right || left.contains(right) || right.contains(left);
    }

    left == right
        || (left.len() >= 4 && right.starts_with(left))
        || (right.len() >= 4 && left.starts_with(right))
}

fn split_normalized_tokens(normalized: &str) -> Vec<String> {
    normalized
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
}

fn expand_semantic_tokens(token: &str, significant_only: bool) -> Vec<String> {
    let mut expanded = BTreeSet::new();
    let stemmed = stem_token(token);
    if !stemmed.is_empty() {
        expanded.insert(stemmed);
    }

    if contains_cjk(token) {
        for width in [2usize, 3usize] {
            for gram in cjk_ngrams(token, width) {
                if !significant_only || gram.chars().count() >= 2 {
                    expanded.insert(gram);
                }
            }
        }
    }

    expanded.into_iter().collect()
}

fn contains_cjk(text: &str) -> bool {
    text.chars().any(is_cjk_character)
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

fn is_han_character(character: char) -> bool {
    matches!(
        character as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    )
}

fn is_japanese_kana_character(character: char) -> bool {
    matches!(character as u32, 0x3040..=0x30ff)
}

fn is_hangul_character(character: char) -> bool {
    matches!(character as u32, 0xac00..=0xd7af)
}

fn cjk_ngrams(token: &str, width: usize) -> Vec<String> {
    let characters = token.chars().collect::<Vec<_>>();
    if characters.len() < width {
        return Vec::new();
    }

    (0..=characters.len() - width)
        .map(|index| characters[index..index + width].iter().collect::<String>())
        .collect()
}

fn anchor_tokens(claim_tokens: &[String]) -> Vec<String> {
    claim_tokens
        .iter()
        .filter(|token| token.len() >= 5)
        .filter(|token| !ANCHOR_STOP_WORDS.contains(&token.as_str()))
        .filter(|token| !QUALIFIER_TOKENS.contains(&token.as_str()))
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn qualifier_tokens(text: &str) -> Vec<String> {
    tokenize_all(text)
        .into_iter()
        .filter(|token| QUALIFIER_TOKENS.contains(&token.as_str()))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn assess_support_guards(
    claim: &ClaimRequest,
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    claim_anchor_tokens: &[String],
    claim_qualifier_tokens: &[String],
) -> GuardAssessment {
    let aggregated_text = aggregate_support_text(top_support, blocks);
    let aggregated_tokens = tokenize_significant(&aggregated_text);
    let aggregated_all_tokens = tokenize_all(&aggregated_text);
    let normalized_claim = normalize_text(&claim.statement);
    let normalized_support = normalize_text(&aggregated_text);
    let mut guard_failures = Vec::new();
    let mut contradiction_reason = None;
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        anchor_guard_check(claim_anchor_tokens, &aggregated_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        qualifier_guard_check(claim_qualifier_tokens, &aggregated_all_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        numeric_guard_check(&claim.statement, &aggregated_text),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        scope_guard_check(&claim.statement, &aggregated_all_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        status_guard_check(&claim.statement, &aggregated_all_tokens),
    );
    apply_guard_check(
        &mut contradiction_reason,
        &mut guard_failures,
        negation_guard_check(&normalized_claim, &normalized_support),
    );

    GuardAssessment {
        contradiction_reason: contradiction_reason.clone(),
        next_action_hint: if contradiction_reason.is_none() {
            next_action_hint_for_failures(&guard_failures)
        } else {
            None
        },
        guard_failures,
    }
}

fn apply_guard_check(
    contradiction_reason: &mut Option<UnsupportedClaimReason>,
    guard_failures: &mut Vec<EvidenceGuardFailure>,
    check: GuardCheck,
) {
    if let Some(reason) = check.contradiction_reason {
        contradiction_reason.get_or_insert(reason);
    }
    if let Some(failure) = check.failure {
        guard_failures.push(failure);
    }
}

fn anchor_guard_check(claim_anchor_tokens: &[String], aggregated_tokens: &[String]) -> GuardCheck {
    let anchor_coverage = if claim_anchor_tokens.is_empty() {
        1.0
    } else {
        token_overlap_ratio(claim_anchor_tokens, aggregated_tokens)
    };
    let required = required_anchor_coverage(claim_anchor_tokens.len());

    if anchor_coverage + 0.001 >= required {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::AnchorCoverage,
            detail: format!(
                "anchor coverage {:.2} is below required {:.2}",
                anchor_coverage, required
            ),
        }),
    }
}

fn qualifier_guard_check(
    claim_qualifier_tokens: &[String],
    aggregated_all_tokens: &[String],
) -> GuardCheck {
    let qualifier_coverage = if claim_qualifier_tokens.is_empty() {
        1.0
    } else {
        token_overlap_ratio(claim_qualifier_tokens, aggregated_all_tokens)
    };

    if qualifier_coverage >= 1.0 {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::QualifierCoverage,
            detail: format!(
                "qualifier coverage {:.2} is below required 1.00",
                qualifier_coverage
            ),
        }),
    }
}

fn numeric_guard_check(claim_text: &str, aggregated_text: &str) -> GuardCheck {
    let claim_numeric = numeric_expressions(claim_text);
    let support_numeric = numeric_expressions(aggregated_text);

    if claim_numeric.is_empty() {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    if support_numeric.is_empty() {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::NumericValue,
                detail: "No exact numeric detail was found in the retrieved support.".to_string(),
            }),
        };
    }

    if numeric_expressions_match(&claim_numeric, &support_numeric) {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: Some(UnsupportedClaimReason::NumericMismatch),
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::NumericValue,
            detail: format!(
                "Claim numeric values {:?} do not match support values {:?}.",
                claim_numeric
                    .iter()
                    .map(NumericExpression::render)
                    .collect::<Vec<_>>(),
                support_numeric
                    .iter()
                    .map(NumericExpression::render)
                    .collect::<Vec<_>>()
            ),
        }),
    }
}

fn scope_guard_check(claim_text: &str, aggregated_all_tokens: &[String]) -> GuardCheck {
    let claim_scope = detect_scope_profile(&tokenize_all(claim_text));
    let support_scope = detect_scope_profile(aggregated_all_tokens);

    if scope_profiles_contradict(claim_scope, support_scope) {
        return GuardCheck {
            contradiction_reason: Some(UnsupportedClaimReason::ScopeMismatch),
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Scope,
                detail: format!(
                    "Claim scope `{}` conflicts with support scope `{}`.",
                    claim_scope.label(),
                    support_scope.label()
                ),
            }),
        };
    }

    if claim_scope.requires_explicit_support() && support_scope == ScopeProfile::Unknown {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Scope,
                detail: "The claim requires explicit scope confirmation, but the support is scope-ambiguous.".to_string(),
            }),
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: None,
    }
}

fn status_guard_check(claim_text: &str, aggregated_all_tokens: &[String]) -> GuardCheck {
    let claim_status = detect_status_profile(&tokenize_all(claim_text));
    let support_status = detect_status_profile(aggregated_all_tokens);

    if status_profiles_contradict(claim_status, support_status) {
        return GuardCheck {
            contradiction_reason: Some(UnsupportedClaimReason::StatusMismatch),
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Status,
                detail: format!(
                    "Claim status `{}` conflicts with support status `{}`.",
                    claim_status.label(),
                    support_status.label()
                ),
            }),
        };
    }

    if claim_status.requires_explicit_support() && support_status == StatusProfile::Unknown {
        return GuardCheck {
            contradiction_reason: None,
            failure: Some(EvidenceGuardFailure {
                kind: EvidenceGuardKind::Status,
                detail: "The claim requires explicit release-status support, but the support is status-ambiguous.".to_string(),
            }),
        };
    }

    GuardCheck {
        contradiction_reason: None,
        failure: None,
    }
}

fn negation_guard_check(normalized_claim: &str, normalized_support: &str) -> GuardCheck {
    if !contradiction_detected(normalized_claim, normalized_support) {
        return GuardCheck {
            contradiction_reason: None,
            failure: None,
        };
    }

    GuardCheck {
        contradiction_reason: Some(UnsupportedClaimReason::NegationMismatch),
        failure: Some(EvidenceGuardFailure {
            kind: EvidenceGuardKind::Negation,
            detail:
                "The retrieved support contains polarity language that conflicts with the claim."
                    .to_string(),
        }),
    }
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

fn append_neighbor_context_text(
    seen_blocks: &mut BTreeSet<String>,
    parts: &mut Vec<String>,
    blocks: &[SnapshotBlock],
    block: &SnapshotBlock,
) {
    let Some(candidate_index) = blocks
        .iter()
        .position(|candidate| std::ptr::eq(candidate, block))
    else {
        return;
    };

    for neighbor_index in contextual_neighbor_indices(blocks, candidate_index) {
        append_unique_block_text(seen_blocks, parts, &blocks[neighbor_index]);
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
    append_neighbor_context_text(seen_blocks, parts, blocks, candidate.block);
}

fn aggregate_support_text(top_support: &[ScoredCandidate<'_>], blocks: &[SnapshotBlock]) -> String {
    let mut seen_blocks = BTreeSet::new();
    let mut parts = Vec::new();

    append_primary_heading_text(&mut seen_blocks, &mut parts, blocks);

    for candidate in top_support {
        append_candidate_support_text(&mut seen_blocks, &mut parts, blocks, candidate);
    }

    parts.join(" ")
}

fn aggregate_support_score(
    claim: &ClaimRequest,
    normalized_claim: &str,
    claim_tokens: &[String],
    claim_numeric_tokens: &[String],
    top_support: &[ScoredCandidate<'_>],
    blocks: &[SnapshotBlock],
    scoring_context: &ScoringContext,
) -> f64 {
    let claim_anchor_tokens = anchor_tokens(&tokenize_significant(&claim.statement));
    let narrative_support_count = top_support
        .iter()
        .filter(|candidate| is_narrative_aggregate_block(candidate.block))
        .count();
    let relevant_primary_heading = primary_heading_context(blocks)
        .filter(|heading| primary_heading_supports_claim(heading, &claim_anchor_tokens));

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
        distributed_support_bonus(&claim.statement, top_support, scoring_context);
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

fn distributed_support_bonus(
    claim_text: &str,
    top_support: &[ScoredCandidate<'_>],
    scoring_context: &ScoringContext,
) -> f64 {
    if top_support.len() < 2 {
        return 0.0;
    }

    let claim_anchor_tokens = anchor_tokens(&tokenize_significant(claim_text));
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
        &claim_anchor_tokens,
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

fn is_narrative_aggregate_block(block: &SnapshotBlock) -> bool {
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

fn primary_heading_supports_claim(heading: &SnapshotBlock, claim_anchor_tokens: &[String]) -> bool {
    if claim_anchor_tokens.is_empty() {
        return false;
    }

    let heading_tokens = tokenize_significant(&heading.text);
    !heading_tokens.is_empty() && token_overlap_ratio(claim_anchor_tokens, &heading_tokens) >= 0.5
}

fn nearest_heading_context<'a>(
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

fn primary_heading_context(blocks: &[SnapshotBlock]) -> Option<&SnapshotBlock> {
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

fn required_anchor_coverage(anchor_count: usize) -> f64 {
    match anchor_count {
        0 => 0.0,
        1 | 2 => 1.0,
        3 => 2.0 / 3.0,
        _ => 0.6,
    }
}

fn should_keep_browsing(
    best_score: f64,
    assessment: &GuardAssessment,
    claim: &ClaimRequest,
) -> bool {
    best_score >= 0.35
        && (!assessment.guard_failures.is_empty()
            || !numeric_expressions(&claim.statement).is_empty()
            || detect_scope_profile(&tokenize_all(&claim.statement)).requires_explicit_support()
            || detect_status_profile(&tokenize_all(&claim.statement)).requires_explicit_support())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NumericExpression {
    value: String,
    unit: Option<String>,
}

impl NumericExpression {
    fn render(&self) -> String {
        match &self.unit {
            Some(unit) => format!("{} {}", self.value, unit),
            None => self.value.clone(),
        }
    }
}

fn numeric_expressions(text: &str) -> Vec<NumericExpression> {
    let tokens = normalize_text(text)
        .split_whitespace()
        .map(|token| token.to_string())
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();

    for (index, token) in tokens.iter().enumerate() {
        if token.chars().all(|character| character.is_ascii_digit()) {
            let expression = NumericExpression {
                value: token.clone(),
                unit: tokens
                    .get(index + 1)
                    .and_then(|candidate| normalize_unit(candidate)),
            };
            if !expressions.contains(&expression) {
                expressions.push(expression);
            }
        }
    }

    expressions
}

fn normalize_unit(token: &str) -> Option<String> {
    match token {
        "second" | "seconds" | "sec" | "secs" => Some("second".to_string()),
        "minute" | "minutes" | "min" | "mins" => Some("minute".to_string()),
        "hour" | "hours" | "hr" | "hrs" => Some("hour".to_string()),
        "day" | "days" => Some("day".to_string()),
        "week" | "weeks" => Some("week".to_string()),
        "month" | "months" => Some("month".to_string()),
        "year" | "years" => Some("year".to_string()),
        _ => None,
    }
}

fn numeric_expressions_match(
    claim_numeric: &[NumericExpression],
    support_numeric: &[NumericExpression],
) -> bool {
    claim_numeric.iter().all(|claim_expr| {
        support_numeric.iter().any(|support_expr| {
            claim_expr.value == support_expr.value
                && match (&claim_expr.unit, &support_expr.unit) {
                    (Some(claim_unit), Some(support_unit)) => claim_unit == support_unit,
                    _ => true,
                }
        })
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScopeProfile {
    Unknown,
    Universal,
    Exclusive,
    Limited,
}

impl ScopeProfile {
    fn requires_explicit_support(self) -> bool {
        matches!(self, ScopeProfile::Universal | ScopeProfile::Exclusive)
    }

    fn label(self) -> &'static str {
        match self {
            ScopeProfile::Unknown => "unknown",
            ScopeProfile::Universal => "universal",
            ScopeProfile::Exclusive => "exclusive",
            ScopeProfile::Limited => "limited",
        }
    }
}

fn detect_scope_profile(tokens: &[String]) -> ScopeProfile {
    if tokens
        .iter()
        .any(|token| UNIVERSAL_SCOPE_TOKENS.contains(&token.as_str()))
    {
        ScopeProfile::Universal
    } else if tokens
        .iter()
        .any(|token| EXCLUSIVE_SCOPE_TOKENS.contains(&token.as_str()))
    {
        ScopeProfile::Exclusive
    } else if tokens
        .iter()
        .any(|token| LIMITED_SCOPE_TOKENS.contains(&token.as_str()))
    {
        ScopeProfile::Limited
    } else {
        ScopeProfile::Unknown
    }
}

fn scope_profiles_contradict(claim: ScopeProfile, support: ScopeProfile) -> bool {
    matches!(
        (claim, support),
        (ScopeProfile::Universal, ScopeProfile::Limited)
            | (ScopeProfile::Universal, ScopeProfile::Exclusive)
            | (ScopeProfile::Exclusive, ScopeProfile::Limited)
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusProfile {
    Unknown,
    Preview,
    GenerallyAvailable,
    Deprecated,
}

impl StatusProfile {
    fn requires_explicit_support(self) -> bool {
        matches!(
            self,
            StatusProfile::Preview | StatusProfile::GenerallyAvailable
        )
    }

    fn label(self) -> &'static str {
        match self {
            StatusProfile::Unknown => "unknown",
            StatusProfile::Preview => "preview",
            StatusProfile::GenerallyAvailable => "general-availability",
            StatusProfile::Deprecated => "deprecated",
        }
    }
}

fn detect_status_profile(tokens: &[String]) -> StatusProfile {
    if tokens
        .iter()
        .any(|token| PREVIEW_STATUS_TOKENS.contains(&token.as_str()))
    {
        StatusProfile::Preview
    } else if tokens
        .iter()
        .any(|token| GA_STATUS_TOKENS.contains(&token.as_str()))
    {
        StatusProfile::GenerallyAvailable
    } else if tokens
        .iter()
        .any(|token| DEPRECATED_STATUS_TOKENS.contains(&token.as_str()))
    {
        StatusProfile::Deprecated
    } else {
        StatusProfile::Unknown
    }
}

fn status_profiles_contradict(claim: StatusProfile, support: StatusProfile) -> bool {
    matches!(
        (claim, support),
        (StatusProfile::GenerallyAvailable, StatusProfile::Preview)
            | (StatusProfile::Preview, StatusProfile::GenerallyAvailable)
            | (StatusProfile::GenerallyAvailable, StatusProfile::Deprecated)
            | (StatusProfile::Preview, StatusProfile::Deprecated)
    )
}

fn next_action_hint_for_failures(failures: &[EvidenceGuardFailure]) -> Option<String> {
    if failures.iter().any(|failure| {
        matches!(
            failure.kind,
            EvidenceGuardKind::NumericValue | EvidenceGuardKind::NumericUnit
        )
    }) {
        Some("Browse the limits, pricing, or quotas page before answering.".to_string())
    } else if failures
        .iter()
        .any(|failure| matches!(failure.kind, EvidenceGuardKind::Scope))
    {
        Some(
            "Browse the regional availability or feature-matrix page before answering.".to_string(),
        )
    } else if failures
        .iter()
        .any(|failure| matches!(failure.kind, EvidenceGuardKind::Status))
    {
        Some("Browse the release notes or feature status page before answering.".to_string())
    } else if failures.is_empty() {
        None
    } else {
        Some("Browse a more specific source page before answering.".to_string())
    }
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "your", "must", "now", "are",
    "all", "per", "there", "page", "include", "includes", "includ", "contain", "contains", "list",
    "built", "flow", "runtime", "plan", "touch", "browser", "both", "modern", "feature", "app",
    "model", "style",
];

const ANCHOR_STOP_WORDS: &[&str] = &[
    "support",
    "avail",
    "available",
    "feature",
    "features",
    "modern",
    "app",
    "apps",
    "model",
    "provid",
    "service",
    "system",
    "platform",
];

const QUALIFIER_TOKENS: &[&str] = &[
    "all",
    "every",
    "fully",
    "native",
    "global",
    "worldwide",
    "only",
    "always",
    "never",
    "entire",
];

const UNIVERSAL_SCOPE_TOKENS: &[&str] =
    &["all", "every", "global", "worldwide", "entire", "universal"];

const EXCLUSIVE_SCOPE_TOKENS: &[&str] = &["only", "exclusive", "solely"];

const LIMITED_SCOPE_TOKENS: &[&str] = &[
    "selected", "some", "certain", "regional", "region", "varies", "subset", "specific",
];

const PREVIEW_STATUS_TOKENS: &[&str] = &["preview", "beta", "alpha", "experimental", "prelaunch"];

const GA_STATUS_TOKENS: &[&str] = &["launched", "generally", "ga"];

const DEPRECATED_STATUS_TOKENS: &[&str] =
    &["deprecated", "legacy", "retired", "sunset", "unsupported"];

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

struct ContradictionPattern {
    positive: &'static str,
    negative: &'static str,
}

const CONTRADICTION_PATTERNS: &[ContradictionPattern] = &[
    ContradictionPattern {
        positive: "available",
        negative: "not available",
    },
    ContradictionPattern {
        positive: "available",
        negative: "unavailable",
    },
    ContradictionPattern {
        positive: "required",
        negative: "not required",
    },
    ContradictionPattern {
        positive: "allowed",
        negative: "not allowed",
    },
    ContradictionPattern {
        positive: "supported",
        negative: "not supported",
    },
    ContradictionPattern {
        positive: "enabled",
        negative: "not enabled",
    },
    ContradictionPattern {
        positive: "synchronous",
        negative: "asynchronous",
    },
    ContradictionPattern {
        positive: "blocking",
        negative: "non blocking",
    },
];

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde::Deserialize;
    use touch_browser_contracts::{
        EvidenceReport, SnapshotDocument, SourceRisk, UnsupportedClaimReason,
    };

    use super::{ClaimRequest, EvidenceExtractor, EvidenceInput};

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureMetadata {
        title: String,
        expected_snapshot_path: String,
        expected_evidence_path: String,
        risk: String,
        expectations: FixtureExpectations,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureExpectations {
        claim_checks: Vec<ClaimCheck>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ClaimCheck {
        id: String,
        statement: String,
    }

    #[test]
    fn produces_expected_evidence_reports_for_seed_fixtures() {
        let extractor = EvidenceExtractor;

        for fixture in seed_fixture_paths() {
            let metadata = read_fixture_metadata(&fixture);
            let snapshot_path = repo_root().join(metadata.expected_snapshot_path);
            let expected_path = repo_root().join(metadata.expected_evidence_path);
            let snapshot: SnapshotDocument = serde_json::from_str(
                &fs::read_to_string(snapshot_path).expect("snapshot should be readable"),
            )
            .expect("snapshot json should deserialize");

            let actual = extractor
                .extract(&EvidenceInput::new(
                    snapshot,
                    metadata
                        .expectations
                        .claim_checks
                        .into_iter()
                        .map(|claim| ClaimRequest::new(claim.id, claim.statement))
                        .collect(),
                    "2026-03-14T00:00:00+09:00",
                    parse_risk(&metadata.risk),
                    Some(metadata.title),
                ))
                .expect("evidence extraction should succeed");

            let expected: EvidenceReport = serde_json::from_str(
                &fs::read_to_string(expected_path).expect("expected evidence should be readable"),
            )
            .expect("expected evidence json should deserialize");

            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn marks_missing_support_as_unsupported() {
        let metadata = read_fixture_metadata(
            &repo_root().join("fixtures/research/static-docs/getting-started/fixture.json"),
        );
        let snapshot_path = repo_root().join(metadata.expected_snapshot_path);
        let snapshot: SnapshotDocument = serde_json::from_str(
            &fs::read_to_string(snapshot_path).expect("snapshot should be readable"),
        )
        .expect("snapshot should deserialize");

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c99",
                    "The page contains a billing checkout form.",
                )],
                "2026-03-14T00:00:00+09:00",
                SourceRisk::Low,
                Some(metadata.title),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert_eq!(report.unsupported_claims.len(), 1);
        assert_eq!(
            report.unsupported_claims[0].reason,
            UnsupportedClaimReason::InsufficientConfidence
        );
    }

    #[test]
    fn marks_contradictory_claim_as_unsupported() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://www.iana.org/help/example-domains".to_string(),
                source_type: touch_browser_contracts::SourceType::Playwright,
                title: Some("Example Domains".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:example-domains-note".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "example.com is not available for registration or transfer. These domains are available for documentation examples."
                    .to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://www.iana.org/help/example-domains".to_string(),
                    source_type: touch_browser_contracts::SourceType::Playwright,
                    dom_path_hint: Some("html > body > main".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "example.com is available for registration or transfer.",
                )],
                "2026-03-17T00:00:00+09:00",
                SourceRisk::Low,
                Some("Example Domains".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert_eq!(report.contradicted_claims.len(), 1);
        assert_eq!(
            report.contradicted_claims[0].reason,
            UnsupportedClaimReason::NegationMismatch
        );
    }

    #[test]
    fn does_not_treat_plural_notes_as_negation() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "fixture://research/navigation/browser-expand".to_string(),
                source_type: touch_browser_contracts::SourceType::Playwright,
                title: Some("Browser Expand".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:details".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "Expanded details confirm that the runtime can reveal collapsed notes."
                    .to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "fixture://research/navigation/browser-expand".to_string(),
                    source_type: touch_browser_contracts::SourceType::Playwright,
                    dom_path_hint: Some("html > body > main".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "Expanded details confirm that the runtime can reveal collapsed notes.",
                )],
                "2026-03-17T00:00:00+09:00",
                SourceRisk::Low,
                Some("Browser Expand".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn rejects_plausible_claims_when_anchor_or_qualifier_coverage_is_missing() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://docs.aws.example/ecs".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Example ECS Overview".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:overview".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Amazon ECS is a fully managed container orchestration service."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/ecs".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:managed-instances".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Managed instances support GPU acceleration for selected workloads."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/ecs".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:regional".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Availability varies by Region and capacity option.".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/ecs".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(3)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![
                    ClaimRequest::new("c1", "ECS supports GPU instances natively."),
                    ClaimRequest::new("c2", "ECS is available in all AWS regions."),
                ],
                "2026-04-05T00:00:00+09:00",
                SourceRisk::Low,
                Some("Example ECS Overview".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert_eq!(
            report.contradicted_claims.len()
                + report.needs_more_browsing_claims.len()
                + report.unsupported_claims.len(),
            2
        );
        assert!(report
            .needs_more_browsing_claims
            .iter()
            .all(|claim| claim.reason == UnsupportedClaimReason::NeedsMoreBrowsing));
    }

    #[test]
    fn supports_claims_when_evidence_is_split_across_heading_and_body_blocks() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://docs.aws.example/ecs/welcome".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("What is Amazon Elastic Container Service?".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 1024,
                estimated_tokens: 128,
                emitted_tokens: 128,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:welcome".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "What is Amazon Elastic Container Service?".to_string(),
                    attributes: serde_json::from_str(r#"{"level":1}"#)
                        .expect("heading attributes should deserialize"),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/ecs/welcome".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:controller".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Controller - Deploy and manage your applications that run on the containers."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/ecs/welcome".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:managed".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Amazon ECS Managed Instances offloads infrastructure management to AWS for containerized workloads."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/ecs/welcome".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "Amazon ECS is a fully managed container orchestration service.",
                )],
                "2026-04-05T00:00:00+09:00",
                SourceRisk::Low,
                Some("What is Amazon Elastic Container Service?".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn does_not_promote_interaction_claims_from_single_button_context() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "fixture://research/navigation/browser-pagination".to_string(),
                source_type: touch_browser_contracts::SourceType::Fixture,
                title: Some("Browser Pagination".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:browser-pagination".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Browser Pagination".to_string(),
                    attributes: serde_json::from_str(r#"{"level":1}"#)
                        .expect("heading attributes should deserialize"),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-pagination".to_string(),
                        source_type: touch_browser_contracts::SourceType::Fixture,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:page-label".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Page 1".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-pagination".to_string(),
                        source_type: touch_browser_contracts::SourceType::Fixture,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:page-content".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Page 1 collects the first batch of release highlights.".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-pagination".to_string(),
                        source_type: touch_browser_contracts::SourceType::Fixture,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b4".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Button,
                    stable_ref: "rmain:button:next".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::FormControl,
                    text: "Next".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-pagination".to_string(),
                        source_type: touch_browser_contracts::SourceType::Fixture,
                        dom_path_hint: Some("html > body > main > button".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", "Page 1 includes a Next button.")],
                "2026-04-05T00:00:00+09:00",
                SourceRisk::Low,
                Some("Browser Pagination".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert!(report.contradicted_claims.is_empty());
        assert_eq!(report.needs_more_browsing_claims.len(), 1);
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn rejects_numeric_mismatches_as_contradicted() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://docs.aws.example/lambda/limits".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Lambda quotas".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:function-configuration".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Function configuration, deployment, and execution".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/lambda/limits".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > h2".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:timeout".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Function timeout: 900 seconds (15 minutes).".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://docs.aws.example/lambda/limits".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "The maximum timeout for a Lambda function is 24 hours.",
                )],
                "2026-04-05T00:00:00+09:00",
                SourceRisk::Low,
                Some("Lambda quotas".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert_eq!(report.contradicted_claims.len(), 1);
        assert_eq!(
            report.contradicted_claims[0].reason,
            UnsupportedClaimReason::NumericMismatch
        );
    }

    #[test]
    fn supports_cjk_claims_when_main_subject_terms_are_present() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://ko.wikipedia.example/wiki/Python".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Python".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:python-origin".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "파이썬은 1991년 귀도 반 로섬이 발표한 프로그래밍 언어이다.".to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://ko.wikipedia.example/wiki/Python".to_string(),
                    source_type: touch_browser_contracts::SourceType::Http,
                    dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "파이썬은 1991년 귀도 반 로섬이 발표한 프로그래밍 언어이다.",
                )],
                "2026-04-05T00:00:00+09:00",
                SourceRisk::Low,
                Some("Python".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn supports_japanese_claims_when_main_subject_terms_are_present() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://ja.wikipedia.example/wiki/明治維新".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("明治維新".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:meiji".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "明治維新は江戸幕府に対する倒幕運動から始まった日本の近代化改革である。"
                    .to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://ja.wikipedia.example/wiki/明治維新".to_string(),
                    source_type: touch_browser_contracts::SourceType::Http,
                    dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "明治維新は江戸幕府に対する倒幕運動から始まった日本の近代化改革である。",
                )],
                "2026-04-05T00:00:00+09:00",
                SourceRisk::Low,
                Some("明治維新".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn supports_simplified_chinese_claims_against_traditional_snapshot_text() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://zh.wikipedia.example/wiki/%E4%B8%AD%E5%9B%BD".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("中國".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 64,
                emitted_tokens: 64,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:china-overview".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "中國是以漢族為主體民族的國家。".to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://zh.wikipedia.example/wiki/%E4%B8%AD%E5%9B%BD".to_string(),
                    source_type: touch_browser_contracts::SourceType::Http,
                    dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", "中国是以汉族为主体民族的国家。")],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("中國".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn supports_paraphrased_claims_from_adjacent_evidence_blocks() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API"
                    .to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Fetch API".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 96,
                emitted_tokens: 96,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:fetch-api".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Fetch API".to_string(),
                    attributes: serde_json::from_str(r#"{"level":1}"#)
                        .expect("heading attributes should deserialize"),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url:
                            "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API"
                                .to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:interface".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "The Fetch API provides an interface for fetching resources."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url:
                            "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API"
                                .to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:promise".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "The fetch() method returns a Promise that resolves to the Response to that request."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url:
                            "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API"
                                .to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "The Fetch API lets JavaScript code request resources and returns a promise-based response model.",
                )],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("Fetch API".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
    }

    #[test]
    fn prefers_main_content_over_navigation_for_js_docs_claims() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://reactrouter.com/home".to_string(),
                source_type: touch_browser_contracts::SourceType::Playwright,
                title: Some("React Router Home".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 128,
                emitted_tokens: 128,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Link,
                    stable_ref: "rnav:link:framework-conventions".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::PrimaryNav,
                    text: "API Framework Conventions".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > nav > a:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:react-router-home".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "React Router Home".to_string(),
                    attributes: serde_json::from_str(r#"{"level":1}"#)
                        .expect("heading attributes should deserialize"),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:intro".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "React Router is a multi-strategy router for React bridging the gap from React 18 to React 19."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b4".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::List,
                    stable_ref: "rmain:list:modes".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "- Framework - Data - Declarative".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > ul".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b5".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:modes-explainer".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "These icons indicate which mode the content is relevant to."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://reactrouter.com/home".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c1",
                    "React Router supports both declarative routing and framework-style features for modern React apps.",
                )],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("React Router Home".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
        assert!(report.claim_outcomes[0]
            .checked_block_refs
            .iter()
            .all(|reference| !reference.starts_with("rnav:")));
    }

    #[test]
    fn rejects_synchronous_runtime_claim_when_support_is_asynchronous() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://nodejs.org/en/about".to_string(),
                source_type: touch_browser_contracts::SourceType::Playwright,
                title: Some("About Node.js".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 96,
                emitted_tokens: 96,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:about-nodejs".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "About Node.js".to_string(),
                    attributes: serde_json::from_str(r#"{"level":1}"#)
                        .expect("heading attributes should deserialize"),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://nodejs.org/en/about".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:runtime-overview".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "As an asynchronous event-driven JavaScript runtime, Node.js is designed to build scalable network applications."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://nodejs.org/en/about".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:standard-library".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "The synchronous methods of the Node.js standard library are convenient for startup tasks."
                        .to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://nodejs.org/en/about".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(2)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", "Node.js is a synchronous runtime.")],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("About Node.js".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert!(report
            .contradicted_claims
            .iter()
            .any(|claim| claim.claim_id == "c1"));
    }

    #[test]
    fn supports_availability_claims_from_selector_options() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://nodejs.org/en/download".to_string(),
                source_type: touch_browser_contracts::SourceType::Playwright,
                title: Some("Node.js Downloads".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 96,
                emitted_tokens: 96,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:downloads".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Node.js Downloads".to_string(),
                    attributes: serde_json::from_str(r#"{"level":1}"#)
                        .expect("heading attributes should deserialize"),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://nodejs.org/en/download".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > h1".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:download-selector".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Choose a platform to download Node.js installers.".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://nodejs.org/en/download".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::List,
                    stable_ref: "rmain:list:platform-options".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Supporting,
                    text: "- macOS - Windows - Linux".to_string(),
                    attributes: serde_json::json!({
                        "zone": "main",
                        "tagName": "listbox",
                        "options": ["macOS", "Windows", "Linux"],
                        "selectionSemantic": "available-options",
                        "textLength": 27
                    })
                    .as_object()
                    .expect("attributes should be an object")
                    .clone()
                    .into_iter()
                    .collect(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://nodejs.org/en/download".to_string(),
                        source_type: touch_browser_contracts::SourceType::Playwright,
                        dom_path_hint: Some("html > body > main > ul[role=listbox]".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", "Node.js is available for macOS.")],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("Node.js Downloads".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert_eq!(report.supported_claims[0].claim_id, "c1");
    }

    fn parse_risk(value: &str) -> SourceRisk {
        match value {
            "low" => SourceRisk::Low,
            "medium" => SourceRisk::Medium,
            "hostile" => SourceRisk::Hostile,
            other => panic!("unknown risk: {other}"),
        }
    }

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()
            .expect("repo root should exist")
    }

    fn seed_fixture_paths() -> Vec<PathBuf> {
        let mut paths = Vec::new();
        collect_fixture_paths(&repo_root().join("fixtures/research"), &mut paths);
        paths.sort();
        paths
    }

    fn read_fixture_metadata(path: &PathBuf) -> FixtureMetadata {
        serde_json::from_str(
            &fs::read_to_string(path).expect("fixture metadata should be readable"),
        )
        .expect("fixture metadata should deserialize")
    }

    fn collect_fixture_paths(root: &PathBuf, paths: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(root).expect("fixture directory should be readable") {
            let entry = entry.expect("fixture directory entry should exist");
            let path = entry.path();
            if path.is_dir() {
                collect_fixture_paths(&path, paths);
            } else if path.file_name().and_then(|name| name.to_str()) == Some("fixture.json") {
                paths.push(path);
            }
        }
    }
}
