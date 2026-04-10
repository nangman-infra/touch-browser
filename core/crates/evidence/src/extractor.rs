use thiserror::Error;
use touch_browser_contracts::{EvidenceReport, SnapshotDocument, SourceRisk};

use crate::reporting::{build_claim_outcome, build_report};

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
