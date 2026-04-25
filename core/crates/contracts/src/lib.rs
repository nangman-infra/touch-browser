use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACTS_CRATE: &str = "touch-browser-contracts";
pub const CONTRACT_VERSION: &str = "1.0.0";
pub const STABLE_REF_VERSION: &str = "1";

pub fn crate_status() -> &'static str {
    "contracts ready"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDocument {
    pub version: String,
    pub stable_ref_version: String,
    pub source: SnapshotSource,
    pub budget: SnapshotBudget,
    pub blocks: Vec<SnapshotBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CompactRefIndexEntry {
    pub id: String,
    #[serde(rename = "ref")]
    pub stable_ref: String,
    pub kind: SnapshotBlockKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotSource {
    pub source_url: String,
    pub source_type: SourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotBudget {
    pub requested_tokens: usize,
    pub estimated_tokens: usize,
    pub emitted_tokens: usize,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaptureDiagnostics {
    pub requested_budget: usize,
    pub effective_budget: usize,
    pub capture_mode: String,
    pub surface: String,
    pub fallback_triggered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
    pub wait_strategy: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_budget_ms: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_consumed_ms: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wait_stop_reason: Option<String>,
    pub quality_score: f64,
    pub quality_label: String,
    pub meaningful_block_count: usize,
    pub main_block_count: usize,
    pub shell_block_count: usize,
    pub truncated: bool,
    pub recommended_next_step: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotBlock {
    pub version: String,
    pub id: String,
    pub kind: SnapshotBlockKind,
    #[serde(rename = "ref")]
    pub stable_ref: String,
    pub role: SnapshotBlockRole,
    pub text: String,
    pub attributes: BTreeMap<String, Value>,
    pub evidence: SnapshotEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotBlockKind {
    Text,
    Heading,
    Link,
    Form,
    Table,
    List,
    Button,
    Input,
    Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SnapshotBlockRole {
    Content,
    PrimaryNav,
    SecondaryNav,
    Cta,
    Metadata,
    Supporting,
    FormControl,
    TableCell,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotEvidence {
    pub source_url: String,
    pub source_type: SourceType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dom_path_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_range_start: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_range_end: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceType {
    Fixture,
    Http,
    Playwright,
    #[serde(rename = "cdp-rust")]
    CdpRust,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SourceRisk {
    Low,
    Medium,
    Hostile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CacheStatus {
    Hit,
    Miss,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionRecord {
    pub version: String,
    pub requested_url: String,
    pub final_url: String,
    pub source_type: SourceType,
    pub status_code: u16,
    pub content_type: String,
    pub redirect_chain: Vec<String>,
    pub cache_status: CacheStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceCitation {
    pub url: String,
    pub retrieved_at: String,
    pub source_type: SourceType,
    pub source_risk: SourceRisk,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceBlock {
    pub version: String,
    pub claim_id: String,
    pub statement: String,
    pub support: Vec<String>,
    #[serde(rename = "supportScore", alias = "confidence")]
    pub support_score: f64,
    pub citation: EvidenceCitation,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_support_snippet: Option<EvidenceSupportSnippet>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub support_snippets: Vec<EvidenceSupportSnippet>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceGuardKind {
    NumericValue,
    NumericUnit,
    Scope,
    Status,
    Negation,
    Predicate,
    AnchorCoverage,
    QualifierCoverage,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceGuardFailure {
    pub kind: EvidenceGuardKind,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UnsupportedClaimReason {
    NoSupportingBlock,
    #[serde(
        rename = "insufficient-support-score",
        alias = "insufficient-confidence"
    )]
    InsufficientConfidence,
    ContradictoryEvidence,
    NumericMismatch,
    ScopeMismatch,
    StatusMismatch,
    NegationMismatch,
    PredicateMismatch,
    NeedsMoreBrowsing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UnsupportedClaim {
    pub version: String,
    pub claim_id: String,
    pub statement: String,
    pub reason: UnsupportedClaimReason,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub checked_block_refs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub guard_failures: Vec<EvidenceGuardFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSource {
    pub source_url: String,
    pub source_type: SourceType,
    pub source_risk: SourceRisk,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceClaimVerdict {
    #[serde(rename = "evidence-supported", alias = "supported")]
    EvidenceSupported,
    Contradicted,
    InsufficientEvidence,
    NeedsMoreBrowsing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceConfidenceBand {
    High,
    Medium,
    Review,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceSupportRole {
    Primary,
    Context,
}

fn default_support_role() -> EvidenceSupportRole {
    EvidenceSupportRole::Context
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceSupportSnippet {
    pub block_id: String,
    pub stable_ref: String,
    pub snippet: String,
    #[serde(default = "default_support_role")]
    pub support_role: EvidenceSupportRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceMatchSignals {
    pub block_id: String,
    pub stable_ref: String,
    pub block_kind: SnapshotBlockKind,
    pub exact_support: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_overlap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contextual_overlap: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numeric_alignment: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_similarity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub semantic_boost: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nli_entailment: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nli_contradiction: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceClaimOutcome {
    pub version: String,
    pub claim_id: String,
    pub statement: String,
    pub verdict: EvidenceClaimVerdict,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub support: Vec<String>,
    #[serde(rename = "supportScore", alias = "confidence")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub support_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub citation: Option<EvidenceCitation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_support_snippet: Option<EvidenceSupportSnippet>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub support_snippets: Vec<EvidenceSupportSnippet>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<UnsupportedClaimReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence_band: Option<EvidenceConfidenceBand>,
    #[serde(default)]
    pub review_recommended: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verdict_explanation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub match_signals: Option<EvidenceMatchSignals>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub checked_block_refs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub guard_failures: Vec<EvidenceGuardFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_action_hint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verification_verdict: Option<EvidenceVerificationVerdict>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceReport {
    pub version: String,
    pub generated_at: String,
    pub source: EvidenceSource,
    #[serde(rename = "evidenceSupportedClaims", alias = "supportedClaims", default)]
    pub supported_claims: Vec<EvidenceBlock>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contradicted_claims: Vec<UnsupportedClaim>,
    #[serde(
        rename = "insufficientEvidenceClaims",
        alias = "unsupportedClaims",
        default
    )]
    pub unsupported_claims: Vec<UnsupportedClaim>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub needs_more_browsing_claims: Vec<UnsupportedClaim>,
    #[serde(default)]
    pub claim_outcomes: Vec<EvidenceClaimOutcome>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub verification: Option<EvidenceVerificationReport>,
}

impl EvidenceClaimOutcome {
    pub fn as_supported_claim(&self) -> Option<EvidenceBlock> {
        (self.verdict == EvidenceClaimVerdict::EvidenceSupported).then(|| EvidenceBlock {
            version: self.version.clone(),
            claim_id: self.claim_id.clone(),
            statement: self.statement.clone(),
            support: self.support.clone(),
            support_score: self.support_score.unwrap_or(0.0),
            citation: self.citation.clone().unwrap_or(EvidenceCitation {
                url: String::new(),
                retrieved_at: String::new(),
                source_type: SourceType::Http,
                source_risk: SourceRisk::Low,
                source_label: None,
            }),
            primary_support_snippet: self
                .primary_support_snippet
                .clone()
                .or_else(|| self.support_snippets.first().cloned()),
            support_snippets: self.support_snippets.clone(),
        })
    }

    pub fn as_issue_claim(&self) -> Option<UnsupportedClaim> {
        (self.verdict != EvidenceClaimVerdict::EvidenceSupported).then(|| UnsupportedClaim {
            version: self.version.clone(),
            claim_id: self.claim_id.clone(),
            statement: self.statement.clone(),
            reason: self
                .reason
                .clone()
                .unwrap_or(UnsupportedClaimReason::InsufficientConfidence),
            checked_block_refs: self.checked_block_refs.clone(),
            guard_failures: self.guard_failures.clone(),
            next_action_hint: self.next_action_hint.clone(),
        })
    }
}

impl EvidenceReport {
    pub fn rebuild_claim_buckets(&mut self) {
        self.supported_claims = self
            .claim_outcomes
            .iter()
            .filter_map(EvidenceClaimOutcome::as_supported_claim)
            .filter(|claim| !claim.citation.url.is_empty())
            .collect();
        self.contradicted_claims = self
            .claim_outcomes
            .iter()
            .filter(|claim| claim.verdict == EvidenceClaimVerdict::Contradicted)
            .filter_map(EvidenceClaimOutcome::as_issue_claim)
            .collect();
        self.unsupported_claims = self
            .claim_outcomes
            .iter()
            .filter(|claim| claim.verdict == EvidenceClaimVerdict::InsufficientEvidence)
            .filter_map(EvidenceClaimOutcome::as_issue_claim)
            .collect();
        self.needs_more_browsing_claims = self
            .claim_outcomes
            .iter()
            .filter(|claim| claim.verdict == EvidenceClaimVerdict::NeedsMoreBrowsing)
            .filter_map(EvidenceClaimOutcome::as_issue_claim)
            .collect();
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EvidenceBlock, EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict,
        EvidenceConfidenceBand, EvidenceMatchSignals, EvidenceSupportRole, EvidenceSupportSnippet,
        RiskClass, SnapshotBlockKind, SourceRisk, SourceType, CONTRACT_VERSION,
    };

    #[test]
    fn evidence_block_accepts_legacy_confidence_field_but_serializes_support_score() {
        let block: EvidenceBlock = serde_json::from_value(serde_json::json!({
            "version": CONTRACT_VERSION,
            "claimId": "c1",
            "statement": "Example",
            "support": ["b1"],
            "confidence": 0.91,
            "citation": {
                "url": "https://example.com",
                "retrievedAt": "2026-03-14T00:00:00+09:00",
                "sourceType": "http",
                "sourceRisk": "low"
            }
        }))
        .expect("legacy confidence payload should deserialize");

        assert_eq!(block.support_score, 0.91);

        let serialized = serde_json::to_value(&block).expect("serialize evidence block");
        assert_eq!(serialized["supportScore"], serde_json::json!(0.91));
        assert!(serialized.get("confidence").is_none());
    }

    #[test]
    fn risk_class_maps_legacy_medium_alias_to_high() {
        let risk: RiskClass = serde_json::from_value(serde_json::json!("medium"))
            .expect("legacy medium should deserialize");
        assert_eq!(risk, RiskClass::High);
        assert_eq!(
            serde_json::to_value(RiskClass::High).expect("serialize risk"),
            serde_json::json!("high")
        );
    }

    #[test]
    fn evidence_block_support_score_round_trips() {
        let block = EvidenceBlock {
            version: CONTRACT_VERSION.to_string(),
            claim_id: "c1".to_string(),
            statement: "Example".to_string(),
            support: vec!["b1".to_string()],
            support_score: 0.88,
            citation: EvidenceCitation {
                url: "https://example.com".to_string(),
                retrieved_at: "2026-03-14T00:00:00+09:00".to_string(),
                source_type: SourceType::Http,
                source_risk: SourceRisk::Low,
                source_label: None,
            },
            primary_support_snippet: None,
            support_snippets: Vec::new(),
        };
        let value = serde_json::to_value(&block).expect("serialize");
        let round_trip: EvidenceBlock =
            serde_json::from_value(value).expect("round trip should succeed");
        assert_eq!(round_trip.support_score, 0.88);
    }

    #[test]
    fn evidence_claim_outcome_serializes_support_snippets_and_confidence_band() {
        let outcome = EvidenceClaimOutcome {
            version: CONTRACT_VERSION.to_string(),
            claim_id: "c1".to_string(),
            statement: "Example".to_string(),
            verdict: EvidenceClaimVerdict::EvidenceSupported,
            support: vec!["b1".to_string()],
            support_score: Some(0.95),
            citation: None,
            primary_support_snippet: Some(EvidenceSupportSnippet {
                block_id: "b1".to_string(),
                stable_ref: "rmain:text:intro".to_string(),
                snippet: "Example Docs supports HTTP snapshots.".to_string(),
                support_role: EvidenceSupportRole::Primary,
            }),
            support_snippets: vec![EvidenceSupportSnippet {
                block_id: "b1".to_string(),
                stable_ref: "rmain:text:intro".to_string(),
                snippet: "Example Docs supports HTTP snapshots.".to_string(),
                support_role: EvidenceSupportRole::Primary,
            }],
            reason: None,
            confidence_band: Some(EvidenceConfidenceBand::High),
            review_recommended: false,
            verdict_explanation: Some(
                "Matched direct support in the page's main content.".to_string(),
            ),
            match_signals: Some(EvidenceMatchSignals {
                block_id: "b1".to_string(),
                stable_ref: "rmain:text:intro".to_string(),
                block_kind: SnapshotBlockKind::Text,
                exact_support: true,
                lexical_overlap: Some(0.91),
                contextual_overlap: Some(0.95),
                numeric_alignment: None,
                semantic_similarity: Some(0.93),
                semantic_boost: Some(0.08),
                nli_entailment: Some(0.97),
                nli_contradiction: Some(0.01),
            }),
            checked_block_refs: vec!["rmain:text:intro".to_string()],
            guard_failures: Vec::new(),
            next_action_hint: None,
            verification_verdict: None,
        };

        let value = serde_json::to_value(&outcome).expect("serialize outcome");
        assert_eq!(value["confidenceBand"], serde_json::json!("high"));
        assert_eq!(
            value["supportSnippets"][0]["blockId"],
            serde_json::json!("b1")
        );
        assert_eq!(
            value["primarySupportSnippet"]["supportRole"],
            serde_json::json!("primary")
        );
        assert_eq!(
            value["supportSnippets"][0]["supportRole"],
            serde_json::json!("primary")
        );
        assert_eq!(
            value["verdictExplanation"],
            serde_json::json!("Matched direct support in the page's main content.")
        );
        assert_eq!(
            value["matchSignals"]["blockKind"],
            serde_json::json!("text")
        );
        assert_eq!(
            value["matchSignals"]["semanticBoost"],
            serde_json::json!(0.08)
        );
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EvidenceVerificationVerdict {
    Verified,
    Contradicted,
    Unresolved,
    NeedsMoreBrowsing,
    InsufficientEvidence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceVerificationOutcome {
    pub version: String,
    pub claim_id: String,
    pub statement: String,
    pub verdict: EvidenceVerificationVerdict,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verifier_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceVerificationReport {
    pub version: String,
    pub verifier: String,
    pub generated_at: String,
    pub outcomes: Vec<EvidenceVerificationOutcome>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SearchEngine {
    Google,
    Brave,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SearchReportStatus {
    Ready,
    Challenge,
    NoResults,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchResultItem {
    pub rank: usize,
    pub title: String,
    pub url: String,
    pub domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snippet: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_ref: Option<String>,
    #[serde(default)]
    pub official_likely: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_surface: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SearchActionActor {
    Ai,
    Human,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchActionHint {
    pub action: String,
    pub detail: String,
    pub actor: SearchActionActor,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<SearchEngine>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default)]
    pub can_auto_run: bool,
    #[serde(default)]
    pub headed_required: bool,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub result_ranks: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchRecoveryAttempt {
    pub engine: SearchEngine,
    pub status: SearchReportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SearchRecovery {
    #[serde(default)]
    pub recovered: bool,
    #[serde(default)]
    pub human_intervention_required_now: bool,
    pub final_engine: SearchEngine,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub attempts: Vec<SearchRecoveryAttempt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SearchReport {
    pub version: String,
    pub generated_at: String,
    pub engine: SearchEngine,
    pub query: String,
    pub search_url: String,
    pub final_url: String,
    pub status: SearchReportStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<SearchRecovery>,
    pub result_count: usize,
    #[serde(default)]
    pub results: Vec<SearchResultItem>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub recommended_result_ranks: Vec<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub next_action_hints: Vec<SearchActionHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActionName {
    Open,
    Read,
    Follow,
    Extract,
    Diff,
    Compact,
    Click,
    Type,
    Submit,
    SelectTab,
    Paginate,
    Expand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RiskClass {
    Low,
    #[serde(alias = "medium")]
    High,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyDecision {
    Allow,
    Review,
    Block,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicySignalKind {
    HostileSource,
    UntrustedSystemLanguage,
    SuspiciousCta,
    ExternalActionable,
    HostileFormControl,
    DomainNotAllowlisted,
    BotChallenge,
    MfaChallenge,
    SensitiveAuthFlow,
    HighRiskWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicySignalOrigin {
    LiveHeuristic,
    FixtureHint,
    PolicyBoundary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicySignal {
    pub kind: PolicySignalKind,
    pub origin: PolicySignalOrigin,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_ref: Option<String>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyRiskSummary {
    pub decision: PolicyDecision,
    pub risk_class: RiskClass,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PolicyReport {
    pub decision: PolicyDecision,
    pub source_risk: SourceRisk,
    pub risk_class: RiskClass,
    pub blocked_refs: Vec<String>,
    pub signals: Vec<PolicySignal>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub allowlisted_domains: Vec<String>,
    pub page_risk: PolicyRiskSummary,
    pub action_risk: PolicyRiskSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionCommand {
    pub version: String,
    pub action: ActionName,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_url: Option<String>,
    pub risk_class: RiskClass,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ActionStatus {
    Succeeded,
    Rejected,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ActionFailureKind {
    UnsupportedAction,
    PolicyBlocked,
    MissingTarget,
    MissingHref,
    UnresolvedLink,
    UnknownSource,
    InvalidInput,
    Internal,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub version: String,
    pub action: ActionName,
    pub status: ActionStatus,
    pub payload_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostics: Option<CaptureDiagnostics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy: Option<PolicyReport>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<ActionFailureKind>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SessionMode {
    ReadOnly,
    Interactive,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Idle,
    Active,
    Completed,
    Blocked,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum PolicyProfile {
    ResearchReadOnly,
    ResearchRestricted,
    InteractiveReview,
    InteractiveSupervisedAuth,
    InteractiveSupervisedWrite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionState {
    pub version: String,
    pub session_id: String,
    pub mode: SessionMode,
    pub status: SessionStatus,
    pub policy_profile: PolicyProfile,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_url: Option<String>,
    pub opened_at: String,
    pub updated_at: String,
    pub visited_urls: Vec<String>,
    pub snapshot_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub working_set_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SessionSynthesisClaimStatus {
    #[serde(rename = "evidence-supported", alias = "supported")]
    EvidenceSupported,
    Contradicted,
    #[serde(rename = "insufficient-evidence", alias = "unsupported")]
    InsufficientEvidence,
    NeedsMoreBrowsing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSynthesisClaim {
    pub version: String,
    pub claim_id: String,
    pub statement: String,
    pub status: SessionSynthesisClaimStatus,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub snapshot_ids: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub support_refs: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub citations: Vec<EvidenceCitation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SessionSynthesisReport {
    pub version: String,
    pub session_id: String,
    pub generated_at: String,
    pub snapshot_count: usize,
    pub evidence_report_count: usize,
    pub visited_urls: Vec<String>,
    pub working_set_refs: Vec<String>,
    pub synthesized_notes: Vec<String>,
    #[serde(rename = "evidenceSupportedClaims", alias = "supportedClaims")]
    pub supported_claims: Vec<SessionSynthesisClaim>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub contradicted_claims: Vec<SessionSynthesisClaim>,
    #[serde(rename = "insufficientEvidenceClaims", alias = "unsupportedClaims")]
    pub unsupported_claims: Vec<SessionSynthesisClaim>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub needs_more_browsing_claims: Vec<SessionSynthesisClaim>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptKind {
    Command,
    Observation,
    Policy,
    System,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TranscriptPayloadType {
    ActionCommand,
    AcquisitionRecord,
    SnapshotBlock,
    SnapshotDocument,
    EvidenceBlock,
    EvidenceReport,
    SessionState,
    JsonRpc,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayTranscriptEntry {
    pub seq: usize,
    pub timestamp: String,
    pub kind: TranscriptKind,
    pub payload_type: TranscriptPayloadType,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ReplayTranscript {
    pub version: String,
    pub session_id: String,
    pub entries: Vec<ReplayTranscriptEntry>,
}
