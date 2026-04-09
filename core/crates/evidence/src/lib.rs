use thiserror::Error;
use touch_browser_contracts::{EvidenceReport, SnapshotDocument, SourceRisk};

mod aggregation;
mod analyzer;
mod candidates;
mod contradiction;
mod normalization;
mod reporting;
mod scoring;
mod segmentation;
mod semantic_matching;

#[cfg(test)]
use contradiction::contradiction_detected;
#[cfg(test)]
use normalization::normalize_text;
use reporting::{build_claim_outcome, build_report};

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

        let claim_outcomes = input
            .claims
            .iter()
            .map(|claim| build_claim_outcome(input, claim))
            .collect();

        Ok(build_report(input, claim_outcomes))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceError {
    #[error("evidence extractor requires at least one claim")]
    NoClaims,
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde::Deserialize;
    use touch_browser_contracts::{
        EvidenceReport, SnapshotDocument, SourceRisk, UnsupportedClaimReason,
    };

    use super::{
        contradiction_detected, normalize_text, ClaimRequest, EvidenceExtractor, EvidenceInput,
    };

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
    fn supports_negative_availability_claim_when_page_matches_negative_polarity() {
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
                    "example.com is not available for registration or transfer.",
                )],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("Example Domains".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
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
    fn defers_table_numeric_noise_to_needs_more_browsing() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Moon landing".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 1024,
                estimated_tokens: 256,
                emitted_tokens: 256,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Heading,
                    stable_ref: "rmain:heading:soviet-uncrewed-soft-landings-1966-1976".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Soviet uncrewed soft landings (1966-1976)".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > h2".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Table,
                    stable_ref:
                        "rmain:table:mission-mass-kg-booster-launch-date-goal-result-"
                            .to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Mission | Mass kg | Booster | Launch date | Goal | Result | Luna 1 | 12 | 20 | 5 | 727 | 14 | 1972 | 0 | 05 | Apollo 11 | 003 | 056 | 8 | 17 | 2221 | 1966 | 1976 | 2 | 41 | 292 | 30 | 000 | 1480 | 1968 | 39 | 3 | 15".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > table".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b3".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::List,
                    stable_ref:
                        "rmain:list:mission-mass-kg-booster-launch-date-goal-result-"
                            .to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "- Apollo 11 - 20 - 1968 - crewed mission - Moon landing - lunar surface operations".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > ul".to_string()),
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
                    "The first crewed Moon landing was Apollo 11 on July 20, 1969.",
                )],
                "2026-04-10T00:00:00+09:00",
                SourceRisk::Low,
                Some("Moon landing".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert!(report.contradicted_claims.is_empty());
        assert_eq!(report.needs_more_browsing_claims.len(), 1);
        assert_eq!(
            report.needs_more_browsing_claims[0].reason,
            UnsupportedClaimReason::NeedsMoreBrowsing
        );
    }

    #[test]
    fn prefers_narrative_support_over_numeric_table_noise_for_date_claims() {
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Moon landing".to_string()),
            },
            budget: touch_browser_contracts::SnapshotBudget {
                requested_tokens: 1024,
                estimated_tokens: 256,
                emitted_tokens: 256,
                truncated: false,
            },
            blocks: vec![
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b1".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Table,
                    stable_ref:
                        "rmain:table:mission-mass-kg-booster-launch-date-goal-result-"
                            .to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Mission | Mass kg | Booster | Launch date | Goal | Result | Luna 1 | 12 | 20 | 5 | 727 | 14 | 1972 | 0 | 05 | Apollo 11 | 003 | 056 | 8 | 17 | 2221 | 1966 | 1976 | 2 | 41 | 292 | 30 | 000 | 1480 | 1968 | 39 | 3 | 15".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
                        source_type: touch_browser_contracts::SourceType::Http,
                        dom_path_hint: Some("html > body > main > table".to_string()),
                        byte_range_start: None,
                        byte_range_end: None,
                    },
                },
                touch_browser_contracts::SnapshotBlock {
                    version: "1.0.0".to_string(),
                    id: "b2".to_string(),
                    kind: touch_browser_contracts::SnapshotBlockKind::Text,
                    stable_ref: "rmain:text:apollo-11-first-crewed-moon-landing".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "Apollo 11 was the first crewed Moon landing on July 20, 1969.".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
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
                    stable_ref: "rmain:text:apollo-11-human-landing".to_string(),
                    role: touch_browser_contracts::SnapshotBlockRole::Content,
                    text: "The mission marked humanity's first landing on the Moon.".to_string(),
                    attributes: Default::default(),
                    evidence: touch_browser_contracts::SnapshotEvidence {
                        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
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
                    "The first crewed Moon landing was Apollo 11 on July 20, 1969.",
                )],
                "2026-04-10T00:00:00+09:00",
                SourceRisk::Low,
                Some("Moon landing".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(
            report.supported_claims[0]
                .support
                .contains(&"b2".to_string()),
            "expected narrative text block to be part of the selected support"
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
    fn supports_asynchronous_runtime_claim_when_synchronous_is_only_contrastive() {
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
                    role: touch_browser_contracts::SnapshotBlockRole::Supporting,
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
                vec![ClaimRequest::new(
                    "c1",
                    "Node.js is an asynchronous runtime.",
                )],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("About Node.js".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(report.supported_claims.len(), 1);
        assert!(report.contradicted_claims.is_empty());
        assert!(report.unsupported_claims.is_empty());
        assert!(report.needs_more_browsing_claims.is_empty());
    }

    #[test]
    fn contradiction_detection_matches_async_polarity_locally() {
        assert!(contradiction_detected(
            &normalize_text("Node.js is a synchronous runtime."),
            "As an asynchronous event-driven JavaScript runtime, Node.js is designed to build scalable network applications."
        ));
        assert!(!contradiction_detected(
            &normalize_text("Node.js is an asynchronous runtime."),
            "The synchronous methods of the Node.js standard library are convenient for startup tasks."
        ));
        assert!(!contradiction_detected(
            &normalize_text("Node.js is an asynchronous runtime."),
            "As an asynchronous event-driven JavaScript runtime, Node.js is designed to build scalable network applications. The synchronous methods of the Node.js standard library are convenient for startup tasks."
        ));
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

    #[test]
    fn rejects_low_signal_repetitive_claims() {
        let repetitive_claim = format!("파이썬은 {}좋은 언어이다", "매우 ".repeat(200));
        let snapshot = SnapshotDocument {
            version: "1.0.0".to_string(),
            stable_ref_version: "1".to_string(),
            source: touch_browser_contracts::SnapshotSource {
                source_url: "https://www.python.org/".to_string(),
                source_type: touch_browser_contracts::SourceType::Http,
                title: Some("Welcome to Python.org".to_string()),
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
                stable_ref: "rmain:text:intro".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "Python is a powerful programming language.".to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://www.python.org/".to_string(),
                    source_type: touch_browser_contracts::SourceType::Http,
                    dom_path_hint: Some("html > body > main > p".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new("c1", repetitive_claim)],
                "2026-04-07T00:00:00+09:00",
                SourceRisk::Low,
                Some("Welcome to Python.org".to_string()),
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
    fn rejects_default_timeout_claim_when_support_only_states_maximum_timeout() {
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
                estimated_tokens: 32,
                emitted_tokens: 32,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:timeout".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "The maximum timeout for a Lambda function is 900 seconds (15 minutes)."
                    .to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://docs.aws.example/lambda/limits".to_string(),
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
                    "The default timeout for a Lambda function is 15 minutes.",
                )],
                "2026-04-09T00:00:00+09:00",
                SourceRisk::Low,
                Some("Lambda quotas".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert_eq!(report.needs_more_browsing_claims.len(), 1);
        assert_eq!(
            report.needs_more_browsing_claims[0].reason,
            UnsupportedClaimReason::NeedsMoreBrowsing
        );
    }

    #[test]
    fn distinguishes_default_and_maximum_claims_inside_the_same_block() {
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
                estimated_tokens: 48,
                emitted_tokens: 48,
                truncated: false,
            },
            blocks: vec![touch_browser_contracts::SnapshotBlock {
                version: "1.0.0".to_string(),
                id: "b1".to_string(),
                kind: touch_browser_contracts::SnapshotBlockKind::Text,
                stable_ref: "rmain:text:timeout".to_string(),
                role: touch_browser_contracts::SnapshotBlockRole::Content,
                text: "The default timeout for a Lambda function is 3 seconds, but you can adjust the Lambda function timeout in increments of 1 second up to a maximum timeout of 900 seconds (15 minutes).".to_string(),
                attributes: Default::default(),
                evidence: touch_browser_contracts::SnapshotEvidence {
                    source_url: "https://docs.aws.example/lambda/limits".to_string(),
                    source_type: touch_browser_contracts::SourceType::Http,
                    dom_path_hint: Some("html > body > main > p:nth-of-type(1)".to_string()),
                    byte_range_start: None,
                    byte_range_end: None,
                },
            }],
        };

        let report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot.clone(),
                vec![ClaimRequest::new(
                    "c1",
                    "The default timeout for a Lambda function is 15 minutes.",
                )],
                "2026-04-09T00:00:00+09:00",
                SourceRisk::Low,
                Some("Lambda quotas".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert!(report.supported_claims.is_empty());
        assert!(
            !report.contradicted_claims.is_empty() || !report.needs_more_browsing_claims.is_empty(),
            "mixed qualifier block should not support the false default claim"
        );

        let maximum_report = EvidenceExtractor
            .extract(&EvidenceInput::new(
                snapshot,
                vec![ClaimRequest::new(
                    "c2",
                    "The maximum timeout for a Lambda function is 15 minutes.",
                )],
                "2026-04-09T00:00:00+09:00",
                SourceRisk::Low,
                Some("Lambda quotas".to_string()),
            ))
            .expect("evidence extraction should succeed");

        assert_eq!(maximum_report.supported_claims.len(), 1);
        assert!(maximum_report.contradicted_claims.is_empty());
    }

    #[test]
    fn contradiction_detection_matches_mutability_polarity_locally() {
        assert!(contradiction_detected(
            &normalize_text("The data structure is mutable."),
            "The data structure is immutable once created.",
        ));
        assert!(!contradiction_detected(
            &normalize_text("The data structure is immutable."),
            "The immutable configuration object can reference mutable caches.",
        ));
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
