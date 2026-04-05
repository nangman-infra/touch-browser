use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const CONTRACTS_CRATE: &str = "touch-browser-contracts";
pub const CONTRACT_VERSION: &str = "1.0.0";
pub const STABLE_REF_VERSION: &str = "1";

pub fn crate_status() -> &'static str {
    "contracts ready"
}

pub fn render_compact_snapshot(snapshot: &SnapshotDocument) -> String {
    let has_heading = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, SnapshotBlockKind::Heading));

    snapshot
        .blocks
        .iter()
        .filter(|block| keep_compact_block(block, has_heading))
        .map(render_compact_block)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_reading_compact_snapshot(snapshot: &SnapshotDocument) -> String {
    let has_heading = snapshot
        .blocks
        .iter()
        .any(|block| matches!(block.kind, SnapshotBlockKind::Heading));

    snapshot
        .blocks
        .iter()
        .filter(|block| keep_reading_block(block, has_heading))
        .map(render_compact_block)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn render_navigation_compact_snapshot(snapshot: &SnapshotDocument) -> String {
    snapshot
        .blocks
        .iter()
        .filter(|block| keep_navigation_block(block))
        .map(render_compact_block)
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn compact_ref_index(snapshot: &SnapshotDocument) -> Vec<CompactRefIndexEntry> {
    snapshot
        .blocks
        .iter()
        .map(|block| CompactRefIndexEntry {
            id: block.id.clone(),
            stable_ref: block.stable_ref.clone(),
            kind: block.kind.clone(),
        })
        .collect()
}

pub fn navigation_ref_index(snapshot: &SnapshotDocument) -> Vec<CompactRefIndexEntry> {
    snapshot
        .blocks
        .iter()
        .filter(|block| keep_navigation_block(block))
        .map(|block| CompactRefIndexEntry {
            id: block.id.clone(),
            stable_ref: block.stable_ref.clone(),
            kind: block.kind.clone(),
        })
        .collect()
}

fn keep_compact_block(block: &SnapshotBlock, has_heading: bool) -> bool {
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
            matches!(block.role, SnapshotBlockRole::Content | SnapshotBlockRole::Cta)
                && is_salient_text_block(&block.text)
        }
        SnapshotBlockKind::Button => {
            matches!(block.role, SnapshotBlockRole::Cta) && is_salient_text_block(&block.text)
        }
        _ => false,
    }
}

fn keep_reading_block(block: &SnapshotBlock, has_heading: bool) -> bool {
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

fn keep_navigation_block(block: &SnapshotBlock) -> bool {
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

fn is_salient_text_block(text: &str) -> bool {
    let word_count = text.split_whitespace().count();
    let lowered = text.to_ascii_lowercase();

    word_count <= 10
        || text.chars().any(|character| character.is_ascii_digit())
        || text.contains('$')
        || text.contains('%')
        || lowered.contains("rfc")
}

fn render_compact_block(block: &SnapshotBlock) -> String {
    let mut parts = vec![compact_kind_code(
        &block.kind,
        block.attributes.get("level").and_then(Value::as_u64),
    )];

    if let Some(attrs) = compact_attr_fragment(block) {
        parts.push(attrs);
    }

    let digest = compact_text_digest(&block.text, &block.kind);
    if !digest.is_empty() {
        parts.push(digest);
    }
    parts.join(" ")
}

fn compact_attr_fragment(block: &SnapshotBlock) -> Option<String> {
    match block.kind {
        SnapshotBlockKind::Link => block
            .attributes
            .get("href")
            .and_then(Value::as_str)
            .and_then(compact_href_fragment)
            .map(|fragment| format!("@{fragment}")),
        SnapshotBlockKind::Input => block
            .attributes
            .get("inputType")
            .and_then(Value::as_str)
            .map(|input_type| format!("={input_type}")),
        SnapshotBlockKind::Table => {
            let rows = block.attributes.get("rows").and_then(Value::as_u64);
            let columns = block.attributes.get("columns").and_then(Value::as_u64);
            match (rows, columns) {
                (Some(rows), Some(columns)) => Some(format!("{rows}x{columns}")),
                (Some(rows), None) => Some(format!("r{rows}")),
                (None, Some(columns)) => Some(format!("c{columns}")),
                (None, None) => None,
            }
        }
        SnapshotBlockKind::List => block
            .attributes
            .get("items")
            .and_then(Value::as_u64)
            .map(|items| format!("n{items}")),
        SnapshotBlockKind::Form => block
            .attributes
            .get("controls")
            .and_then(Value::as_u64)
            .map(|controls| format!("n{controls}")),
        _ => None,
    }
}

fn compact_href_fragment(href: &str) -> Option<String> {
    if let Some(rest) = href
        .strip_prefix("https://")
        .or_else(|| href.strip_prefix("http://"))
    {
        return Some(
            rest.split('/')
                .next()
                .unwrap_or(rest)
                .trim_end_matches('/')
                .to_string(),
        );
    }

    if let Some(email) = href.strip_prefix("mailto:") {
        return Some(email.to_string());
    }

    if let Some(phone) = href.strip_prefix("tel:") {
        return Some(phone.to_string());
    }

    None
}

fn compact_text_digest(text: &str, kind: &SnapshotBlockKind) -> String {
    let normalized = text.split_whitespace().collect::<Vec<_>>();
    if normalized.is_empty() {
        return String::new();
    }

    let actionable = matches!(
        kind,
        SnapshotBlockKind::Heading
            | SnapshotBlockKind::Link
            | SnapshotBlockKind::Button
            | SnapshotBlockKind::Input
    );
    let limit = match kind {
        SnapshotBlockKind::Heading => 2,
        SnapshotBlockKind::Link | SnapshotBlockKind::Button => 2,
        SnapshotBlockKind::Input => 1,
        SnapshotBlockKind::Table => 3,
        _ => 2,
    };

    let mut kept = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for token in &normalized {
        let cleaned = compact_token(token);
        if cleaned.is_empty() {
            continue;
        }

        let lowered = cleaned.to_ascii_lowercase();
        if !seen.insert(lowered.clone()) {
            continue;
        }

        if actionable || is_signal_token(&cleaned, &lowered) {
            kept.push(truncate_compact_token(&cleaned));
        }

        if kept.len() >= limit {
            break;
        }
    }

    if kept.is_empty() {
        kept = normalized
            .iter()
            .map(|token| compact_token(token))
            .filter(|token| !token.is_empty())
            .take(limit)
            .map(|token| truncate_compact_token(&token))
            .collect();
    }

    kept.join(" ")
}

fn compact_token(token: &str) -> String {
    token
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric()
                && character != '$'
                && character != '%'
                && character != '/'
                && character != '.'
                && character != ':'
                && character != '-'
                && character != '_'
        })
        .to_string()
}

fn truncate_compact_token(token: &str) -> String {
    let max_chars = 12usize;
    if token.chars().count() <= max_chars {
        return token.to_string();
    }

    token.chars().take(max_chars).collect()
}

fn is_signal_token(token: &str, lowered: &str) -> bool {
    token.chars().any(|character| character.is_ascii_digit())
        || token.starts_with('$')
        || token.contains('%')
        || token.contains('/')
        || token.contains(':')
        || (!is_stopword(lowered) && token.len() > 5)
}

fn is_stopword(token: &str) -> bool {
    matches!(
        token,
        "a" | "an"
            | "and"
            | "are"
            | "as"
            | "at"
            | "be"
            | "by"
            | "for"
            | "from"
            | "in"
            | "into"
            | "is"
            | "of"
            | "on"
            | "or"
            | "that"
            | "the"
            | "this"
            | "to"
            | "with"
    )
}

fn compact_kind_code(kind: &SnapshotBlockKind, level: Option<u64>) -> String {
    match kind {
        SnapshotBlockKind::Metadata => "m".to_string(),
        SnapshotBlockKind::Heading => format!("h{}", level.unwrap_or(0)),
        SnapshotBlockKind::Text => "t".to_string(),
        SnapshotBlockKind::Link => "a".to_string(),
        SnapshotBlockKind::List => "l".to_string(),
        SnapshotBlockKind::Table => "tb".to_string(),
        SnapshotBlockKind::Form => "f".to_string(),
        SnapshotBlockKind::Button => "b".to_string(),
        SnapshotBlockKind::Input => "i".to_string(),
    }
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
    pub confidence: f64,
    pub citation: EvidenceCitation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum UnsupportedClaimReason {
    NoSupportingBlock,
    InsufficientConfidence,
    ContradictoryEvidence,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceReport {
    pub version: String,
    pub generated_at: String,
    pub source: EvidenceSource,
    pub supported_claims: Vec<EvidenceBlock>,
    pub unsupported_claims: Vec<UnsupportedClaim>,
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
    Medium,
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
#[serde(rename_all = "camelCase")]
pub struct PolicySignal {
    pub kind: PolicySignalKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stable_ref: Option<String>,
    pub detail: String,
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

#[cfg(test)]
mod tests {
    use super::{
        compact_ref_index, render_compact_snapshot, SnapshotBlock, SnapshotBlockKind,
        SnapshotBlockRole, SnapshotBudget, SnapshotDocument, SnapshotEvidence, SnapshotSource,
        SourceType, CONTRACT_VERSION, STABLE_REF_VERSION,
    };
    use serde_json::json;
    use std::collections::BTreeMap;

    #[test]
    fn renders_compact_snapshot_lines() {
        let snapshot = SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: "fixture://research/navigation/browser-follow".to_string(),
                source_type: SourceType::Fixture,
                title: Some("Browser Follow".to_string()),
            },
            budget: SnapshotBudget {
                requested_tokens: 512,
                estimated_tokens: 24,
                emitted_tokens: 24,
                truncated: false,
            },
            blocks: vec![
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b1".to_string(),
                    kind: SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:browser-follow".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Browser Follow".to_string(),
                    attributes: BTreeMap::from([("level".to_string(), json!(1))]),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                SnapshotBlock {
                    version: CONTRACT_VERSION.to_string(),
                    id: "b2".to_string(),
                    kind: SnapshotBlockKind::Link,
                    stable_ref: "rmain:link:docs".to_string(),
                    role: SnapshotBlockRole::Content,
                    text: "Open docs".to_string(),
                    attributes: BTreeMap::from([("href".to_string(), json!("#docs"))]),
                    evidence: SnapshotEvidence {
                        source_url: "fixture://research/navigation/browser-follow".to_string(),
                        source_type: SourceType::Fixture,
                        dom_path_hint: Some("html > body > main".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
            ],
        };

        assert_eq!(
            render_compact_snapshot(&snapshot),
            "h1 Browser Follow\na Open docs"
        );
        assert_eq!(
            compact_ref_index(&snapshot)
                .into_iter()
                .map(|entry| (entry.id, entry.stable_ref))
                .collect::<Vec<_>>(),
            vec![
                ("b1".to_string(), "rmain:heading:browser-follow".to_string()),
                ("b2".to_string(), "rmain:link:docs".to_string()),
            ]
        );
    }
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
    Supported,
    Unsupported,
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
    pub supported_claims: Vec<SessionSynthesisClaim>,
    pub unsupported_claims: Vec<SessionSynthesisClaim>,
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
