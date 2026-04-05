use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use touch_browser_contracts::{SessionState, SourceRisk, SourceType, TranscriptKind};
use touch_browser_memory::{plan_memory_turn, summarize_turns, MemorySessionSummary};
use touch_browser_runtime::{CatalogDocument, ClaimInput, FixtureCatalog, ReadOnlyRuntime};

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
struct MemoryScenarioOutput {
    requested_actions: usize,
    action_count: usize,
    session_state: SessionState,
    memory_summary: MemorySessionSummary,
}

fn main() {
    let requested_actions = std::env::var("TOUCH_BROWSER_MEMORY_ACTIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(20);
    assert!(
        requested_actions > 0 && requested_actions.is_multiple_of(2),
        "TOUCH_BROWSER_MEMORY_ACTIONS must be an even number greater than 0"
    );

    let runtime = ReadOnlyRuntime::default();
    let catalog = fixture_catalog();
    let mut session = runtime.start_session("smemoryscenario001", "2026-03-14T00:00:00+09:00");
    let sequence = [
        (
            "fixture://research/static-docs/getting-started",
            "docs",
            "Touch Browser compiles web pages into semantic state for research agents.",
        ),
        (
            "fixture://research/citation-heavy/pricing",
            "pricing",
            "The Starter plan costs $29 per month.",
        ),
        (
            "fixture://research/navigation/api-reference",
            "api",
            "Snapshot responses include stable refs and evidence metadata.",
        ),
    ];

    let mut memory_refs = Vec::new();
    let mut memory_turns = Vec::new();

    for pair_index in 0..(requested_actions / 2) {
        let step = sequence[pair_index % sequence.len()];
        let open_timestamp = slot_timestamp(pair_index * 2, 0);
        let extract_timestamp = slot_timestamp(pair_index * 2 + 1, 30);

        runtime
            .open(&mut session, &catalog, step.0, &open_timestamp)
            .expect("open should work");
        let snapshot_record = session.snapshots.last().expect("snapshot after open");
        let open_turn = plan_memory_turn(
            memory_turns.len() + 1,
            &snapshot_record.snapshot_id,
            &snapshot_record.snapshot,
            None,
            &memory_refs,
            6,
        );
        memory_refs = open_turn.kept_refs.clone();
        memory_turns.push(open_turn);

        runtime
            .extract(
                &mut session,
                vec![ClaimInput {
                    id: format!("{}-{pair_index}", step.1),
                    statement: step.2.to_string(),
                }],
                &extract_timestamp,
            )
            .expect("extract should work");
        let snapshot_record = session
            .snapshots
            .last()
            .expect("snapshot should remain current after extract");
        let report = session
            .evidence_reports
            .last()
            .expect("evidence report should exist");
        let extract_turn = plan_memory_turn(
            memory_turns.len() + 1,
            &snapshot_record.snapshot_id,
            &snapshot_record.snapshot,
            Some(&report.report),
            &memory_refs,
            6,
        );
        memory_refs = extract_turn.kept_refs.clone();
        memory_turns.push(extract_turn);
    }

    let action_count = session
        .transcript
        .entries
        .iter()
        .filter(|entry| entry.kind == TranscriptKind::Command)
        .count();

    println!(
        "{}",
        serde_json::to_string_pretty(&MemoryScenarioOutput {
            requested_actions,
            action_count,
            session_state: session.state,
            memory_summary: summarize_turns(&memory_turns, 12),
        })
        .expect("memory scenario should serialize")
    );
}

fn slot_timestamp(slot: usize, seconds: usize) -> String {
    let hour = slot / 60;
    let minute = slot % 60;
    format!("2026-03-14T{hour:02}:{minute:02}:{seconds:02}+09:00")
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
