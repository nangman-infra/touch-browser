use std::{env, fs};

use serde::Deserialize;
use touch_browser_contracts::{SnapshotDocument, SourceRisk};
use touch_browser_evidence::{ClaimRequest, EvidenceExtractor, EvidenceInput};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMetadata {
    title: String,
    expected_snapshot_path: String,
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

fn main() {
    let metadata_path = env::args()
        .nth(1)
        .expect("usage: render_fixture_evidence <fixture_metadata_path>");
    let metadata: FixtureMetadata = serde_json::from_str(
        &fs::read_to_string(&metadata_path).expect("fixture metadata should be readable"),
    )
    .expect("fixture metadata should deserialize");
    let snapshot_path = env::current_dir()
        .expect("cwd should be available")
        .join(metadata.expected_snapshot_path);
    let snapshot: SnapshotDocument = serde_json::from_str(
        &fs::read_to_string(snapshot_path).expect("snapshot should be readable"),
    )
    .expect("snapshot json should deserialize");

    let report = EvidenceExtractor
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

    println!(
        "{}",
        serde_json::to_string_pretty(&report).expect("report should serialize")
    );
}

fn parse_risk(value: &str) -> SourceRisk {
    match value {
        "low" => SourceRisk::Low,
        "medium" => SourceRisk::Medium,
        "hostile" => SourceRisk::Hostile,
        other => panic!("unknown risk: {other}"),
    }
}
