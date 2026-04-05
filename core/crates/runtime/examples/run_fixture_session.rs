use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use touch_browser_contracts::{ReplayTranscript, SessionState, SourceRisk, SourceType};
use touch_browser_runtime::{
    CatalogDocument, ClaimInput, CompactInput, DiffInput, FixtureCatalog, ReadOnlyRuntime,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMetadata {
    title: String,
    source_uri: String,
    html_path: String,
    risk: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionScenarioOutput {
    session_state: SessionState,
    replay_transcript: ReplayTranscript,
}

fn main() {
    let runtime = ReadOnlyRuntime::default();
    let catalog = fixture_catalog();
    let mut session = runtime.start_session("sscenario001", "2026-03-14T00:00:00+09:00");

    runtime
        .open(
            &mut session,
            &catalog,
            "fixture://research/static-docs/getting-started",
            "2026-03-14T00:00:01+09:00",
        )
        .expect("open should work");
    runtime
        .read(&mut session, "2026-03-14T00:00:02+09:00")
        .expect("read should work");
    runtime
        .follow(
            &mut session,
            &catalog,
            "rnav:link:pricing",
            "2026-03-14T00:00:03+09:00",
        )
        .expect("follow should work");
    runtime
        .extract(
            &mut session,
            vec![
                ClaimInput {
                    id: "c1".to_string(),
                    statement: "The Starter plan costs $29 per month.".to_string(),
                },
                ClaimInput {
                    id: "c2".to_string(),
                    statement: "There is an Enterprise plan.".to_string(),
                },
            ],
            "2026-03-14T00:00:04+09:00",
        )
        .expect("extract should work");
    runtime
        .diff(
            &mut session,
            DiffInput {
                from_snapshot_id: "snap_scenario001_1".to_string(),
                to_snapshot_id: "snap_scenario001_2".to_string(),
            },
            "2026-03-14T00:00:05+09:00",
        )
        .expect("diff should work");
    runtime
        .compact(
            &mut session,
            CompactInput { limit: 3 },
            "2026-03-14T00:00:06+09:00",
        )
        .expect("compact should work");

    println!(
        "{}",
        serde_json::to_string_pretty(&SessionScenarioOutput {
            session_state: session.state,
            replay_transcript: session.transcript,
        })
        .expect("scenario output should serialize")
    );
}

fn fixture_catalog() -> FixtureCatalog {
    let mut catalog = FixtureCatalog::default();

    for fixture_path in fixture_metadata_paths() {
        let metadata = read_fixture_metadata(&fixture_path);
        let html_path = repo_root().join(metadata.html_path);
        let html = fs::read_to_string(html_path).expect("fixture html should be readable");
        let risk = match metadata.risk.as_str() {
            "low" => SourceRisk::Low,
            "medium" => SourceRisk::Medium,
            "hostile" => SourceRisk::Hostile,
            other => panic!("unexpected risk: {other}"),
        };
        let aliases = match metadata.source_uri.as_str() {
            "fixture://research/static-docs/getting-started" => {
                vec!["/docs".to_string(), "/getting-started".to_string()]
            }
            "fixture://research/citation-heavy/pricing" => vec!["/pricing".to_string()],
            "fixture://research/navigation/api-reference" => {
                vec!["/api".to_string(), "/api-reference".to_string()]
            }
            _ => Vec::new(),
        };

        catalog.register(
            CatalogDocument::new(
                metadata.source_uri,
                html,
                SourceType::Fixture,
                risk,
                Some(metadata.title),
            )
            .with_aliases(aliases),
        );
    }

    catalog
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should exist")
}

fn fixture_metadata_paths() -> Vec<PathBuf> {
    vec![
        repo_root().join("fixtures/research/static-docs/getting-started/fixture.json"),
        repo_root().join("fixtures/research/navigation/api-reference/fixture.json"),
        repo_root().join("fixtures/research/citation-heavy/pricing/fixture.json"),
    ]
}

fn read_fixture_metadata(path: &PathBuf) -> FixtureMetadata {
    serde_json::from_str(&fs::read_to_string(path).expect("fixture metadata should be readable"))
        .expect("fixture metadata should deserialize")
}
