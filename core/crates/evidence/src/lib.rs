use std::collections::BTreeSet;

use thiserror::Error;
use touch_browser_contracts::{
    EvidenceBlock, EvidenceCitation, EvidenceReport, EvidenceSource, SnapshotBlock,
    SnapshotBlockKind, SnapshotDocument, SourceRisk, UnsupportedClaim, UnsupportedClaimReason,
    CONTRACT_VERSION,
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

        let mut supported_claims = Vec::new();
        let mut unsupported_claims = Vec::new();

        for claim in &input.claims {
            let analysis = analyze_claim(claim, &input.snapshot.blocks);

            match analysis {
                ClaimAnalysis::Supported {
                    support,
                    confidence,
                } => {
                    supported_claims.push(EvidenceBlock {
                        version: CONTRACT_VERSION.to_string(),
                        claim_id: claim.claim_id.clone(),
                        statement: claim.statement.clone(),
                        support: support
                            .iter()
                            .map(|candidate| candidate.block.id.clone())
                            .collect(),
                        confidence,
                        citation: EvidenceCitation {
                            url: input.snapshot.source.source_url.clone(),
                            retrieved_at: input.generated_at.clone(),
                            source_type: input.snapshot.source.source_type.clone(),
                            source_risk: input.source_risk.clone(),
                            source_label: input
                                .source_label
                                .clone()
                                .or_else(|| input.snapshot.source.title.clone()),
                        },
                    });
                }
                ClaimAnalysis::Unsupported {
                    reason,
                    checked_refs,
                } => {
                    unsupported_claims.push(UnsupportedClaim {
                        version: CONTRACT_VERSION.to_string(),
                        claim_id: claim.claim_id.clone(),
                        statement: claim.statement.clone(),
                        reason,
                        checked_block_refs: checked_refs,
                    });
                }
            }
        }

        Ok(EvidenceReport {
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
            supported_claims,
            unsupported_claims,
            verification: None,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceError {
    #[error("evidence extractor requires at least one claim")]
    NoClaims,
}

enum ClaimAnalysis<'a> {
    Supported {
        support: Vec<ScoredCandidate<'a>>,
        confidence: f64,
    },
    Unsupported {
        reason: UnsupportedClaimReason,
        checked_refs: Vec<String>,
    },
}

#[derive(Clone)]
struct ScoredCandidate<'a> {
    block: &'a SnapshotBlock,
    score: f64,
    contradictory: bool,
}

fn analyze_claim<'a>(claim: &ClaimRequest, blocks: &'a [SnapshotBlock]) -> ClaimAnalysis<'a> {
    let claim_tokens = tokenize_significant(&claim.statement);
    let claim_numeric_tokens = numeric_tokens(&claim.statement);
    let normalized_claim = normalize_text(&claim.statement);

    let mut scored = blocks
        .iter()
        .filter_map(|block| {
            score_block(
                block,
                &normalized_claim,
                &claim_tokens,
                &claim_numeric_tokens,
            )
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let checked_refs = scored
        .iter()
        .take(3)
        .map(|candidate| candidate.block.stable_ref.clone())
        .collect::<Vec<_>>();

    let contradictory_exists = scored.iter().any(|candidate| candidate.contradictory);

    let non_contradictory = scored
        .into_iter()
        .filter(|candidate| !candidate.contradictory)
        .collect::<Vec<_>>();

    let Some(best_candidate) = non_contradictory.first() else {
        if contradictory_exists {
            return ClaimAnalysis::Unsupported {
                reason: UnsupportedClaimReason::ContradictoryEvidence,
                checked_refs,
            };
        }
        return ClaimAnalysis::Unsupported {
            reason: UnsupportedClaimReason::NoSupportingBlock,
            checked_refs,
        };
    };
    let best_score = best_candidate.score;

    if best_score < 0.52 {
        return ClaimAnalysis::Unsupported {
            reason: UnsupportedClaimReason::InsufficientConfidence,
            checked_refs,
        };
    }

    let top_support = non_contradictory
        .into_iter()
        .filter(|candidate| candidate.score >= 0.33)
        .take(3)
        .collect::<Vec<_>>();

    if top_support.is_empty() {
        let reason = if contradictory_exists {
            UnsupportedClaimReason::ContradictoryEvidence
        } else {
            UnsupportedClaimReason::InsufficientConfidence
        };

        return ClaimAnalysis::Unsupported {
            reason,
            checked_refs,
        };
    }

    let confidence = round_confidence(
        best_score.max(
            top_support
                .iter()
                .map(|candidate| candidate.score)
                .fold(0.0, f64::max),
        ),
    );

    ClaimAnalysis::Supported {
        support: top_support,
        confidence,
    }
}

fn score_block<'a>(
    block: &'a SnapshotBlock,
    normalized_claim: &str,
    claim_tokens: &[String],
    claim_numeric_tokens: &[String],
) -> Option<ScoredCandidate<'a>> {
    let search_text = block_search_text(block);
    let block_tokens = tokenize_significant(&search_text);
    if block_tokens.is_empty() {
        return None;
    }

    let lexical_overlap = token_overlap_ratio(claim_tokens, &block_tokens);
    let exact_bonus = exact_match_bonus(normalized_claim, &normalize_text(&search_text));
    let numeric_overlap = numeric_overlap_ratio(claim_numeric_tokens, &search_text);
    let kind_bonus = kind_score_bonus(&block.kind);
    let contradictory = contradiction_detected(normalized_claim, &search_text)
        || contradiction_detected(normalized_claim, &block.text);
    let mut score =
        (lexical_overlap * 0.72) + (exact_bonus * 0.18) + (numeric_overlap * 0.10) + kind_bonus;

    if contradictory && lexical_overlap >= 0.4 {
        score *= 0.2;
    }

    (score > 0.0).then_some(ScoredCandidate {
        block,
        score: score.min(1.0),
        contradictory,
    })
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
    text.contains(phrase)
}

fn block_search_text(block: &SnapshotBlock) -> String {
    let mut parts = vec![block.text.clone()];

    for (key, value) in &block.attributes {
        match value {
            serde_json::Value::String(text) => parts.push(text.clone()),
            serde_json::Value::Bool(true) => parts.push(key.clone()),
            serde_json::Value::Number(number) => parts.push(number.to_string()),
            _ => {}
        }
    }

    parts.extend(block_semantic_terms(block));

    parts.join(" ")
}

fn block_semantic_terms(block: &SnapshotBlock) -> Vec<String> {
    let mut parts = Vec::new();
    let normalized_text = normalize_text(&block.text);

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

fn exact_match_bonus(normalized_claim: &str, normalized_block_text: &str) -> f64 {
    if normalized_claim.is_empty() || normalized_block_text.is_empty() {
        return 0.0;
    }

    if normalized_block_text.contains(normalized_claim) {
        1.0
    } else if normalized_claim
        .split_whitespace()
        .all(|token| normalized_block_text.contains(token))
    {
        0.55
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

fn round_confidence(score: f64) -> f64 {
    let confidence = 0.55 + (score * 0.40);
    (confidence * 100.0).round() / 100.0
}

fn tokenize_significant(text: &str) -> Vec<String> {
    normalize_text(text)
        .split_whitespace()
        .map(stem_token)
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

fn normalize_text(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());

    for character in text.chars().flat_map(|character| character.to_lowercase()) {
        if character.is_ascii_alphanumeric() {
            normalized.push(character);
        } else {
            normalized.push(' ');
        }
    }

    normalized.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn stem_token(token: &str) -> String {
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

    token.len() >= 3 && !STOP_WORDS.contains(&token)
}

fn tokens_match(left: &str, right: &str) -> bool {
    left == right
        || (left.len() >= 4 && right.starts_with(left))
        || (right.len() >= 4 && left.starts_with(right))
}

const STOP_WORDS: &[&str] = &[
    "the", "and", "for", "with", "that", "this", "from", "into", "your", "must", "now", "are",
    "all", "per", "there", "page", "include", "includes", "includ", "contain", "contains", "list",
    "built", "flow", "runtime", "plan", "touch", "browser",
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
                text: "These domains are available for documentation and are not available for registration or transfer.".to_string(),
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
                    "example.com is available for general registration.",
                )],
                "2026-03-17T00:00:00+09:00",
                SourceRisk::Low,
                Some("Example Domains".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert_eq!(report.unsupported_claims.len(), 1);
        assert_eq!(
            report.unsupported_claims[0].reason,
            UnsupportedClaimReason::ContradictoryEvidence
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
        assert!(report.unsupported_claims.is_empty());
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
