use std::{
    fs,
    io::Cursor,
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc,
    },
    thread,
};

use serde::Deserialize;
use tiny_http::{Header, Response as TinyResponse, Server, StatusCode};
use touch_browser_acquisition::{AcquisitionConfig, AcquisitionEngine};
use touch_browser_contracts::{
    EvidenceCitation, EvidenceClaimOutcome, EvidenceClaimVerdict, EvidenceReport, EvidenceSource,
    ReplayTranscript, SessionMode, SessionStatus, SourceRisk, SourceType, TranscriptKind,
    TranscriptPayloadType, UnsupportedClaimReason, CONTRACT_VERSION,
};
use touch_browser_memory::{plan_memory_turn, summarize_turns};

use super::{
    CatalogDocument, ClaimInput, CompactInput, DiffInput, EvidenceRecord, FixtureCatalog,
    ReadOnlyRuntime,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMetadata {
    title: String,
    source_uri: String,
    html_path: String,
    risk: String,
}

#[test]
fn executes_read_only_fixture_session_and_replays_deterministically() {
    let runtime = ReadOnlyRuntime::default();
    let catalog = fixture_catalog();
    let mut session = runtime.start_session("sfixture001", "2026-03-14T00:00:00+09:00");

    let opened = runtime
        .open(
            &mut session,
            &catalog,
            "fixture://research/static-docs/getting-started",
            "2026-03-14T00:00:01+09:00",
        )
        .expect("open should work");
    assert_eq!(
        opened.source.source_url,
        "fixture://research/static-docs/getting-started"
    );
    assert_eq!(session.state.mode, SessionMode::ReadOnly);
    assert_eq!(session.state.status, SessionStatus::Active);

    let _ = runtime
        .read(&mut session, "2026-03-14T00:00:02+09:00")
        .expect("read should work");
    let followed = runtime
        .follow(
            &mut session,
            &catalog,
            "rnav:link:pricing",
            "2026-03-14T00:00:03+09:00",
        )
        .expect("follow should work");
    assert_eq!(
        followed.source.source_url,
        "fixture://research/citation-heavy/pricing"
    );

    let report = runtime
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
    assert_eq!(report.supported_claims.len(), 1);
    assert_eq!(report.unsupported_claims.len(), 1);

    let diff = runtime
        .diff(
            &mut session,
            DiffInput {
                from_snapshot_id: "snap_fixture001_1".to_string(),
                to_snapshot_id: "snap_fixture001_2".to_string(),
            },
            "2026-03-14T00:00:05+09:00",
        )
        .expect("diff should work");
    assert!(diff
        .added_refs
        .contains(&"rmain:table:plan-monthly-price-snapshots-starter-29-10-000-t".to_string()));

    let compacted = runtime
        .compact(
            &mut session,
            CompactInput { limit: 3 },
            "2026-03-14T00:00:06+09:00",
        )
        .expect("compact should work");
    assert_eq!(compacted.kept_refs.len(), 3);

    let transcript_json =
        serde_json::to_string_pretty(&session.transcript).expect("transcript serialize");
    let replay_transcript: ReplayTranscript =
        serde_json::from_str(&transcript_json).expect("transcript deserialize");
    let replayed = runtime
        .replay(&catalog, &replay_transcript, "2026-03-14T00:00:00+09:00")
        .expect("replay should work");

    assert_eq!(session.state, replayed.state);
    assert_eq!(session.snapshots, replayed.snapshots);
    assert_eq!(session.evidence_reports, replayed.evidence_reports);

    let action_entries = session
        .transcript
        .entries
        .iter()
        .filter(|entry| entry.kind == TranscriptKind::Command)
        .collect::<Vec<_>>();
    assert_eq!(action_entries.len(), 6);
    assert!(session
        .transcript
        .entries
        .iter()
        .any(|entry| entry.payload_type == TranscriptPayloadType::EvidenceReport));
}

#[test]
fn opens_live_documents_via_acquisition_and_records_fetch_metadata() {
    let runtime = ReadOnlyRuntime::default();
    let mut acquisition =
        AcquisitionEngine::new(AcquisitionConfig::default()).expect("acquisition");
    let requests = Arc::new(AtomicUsize::new(0));
    let server = LiveTestServer::start(requests.clone());
    let mut session = runtime.start_session("slive001", "2026-03-14T00:00:00+09:00");

    let snapshot = runtime
        .open_live(
            &mut session,
            &mut acquisition,
            &format!("{}/start#intro", server.base_url()),
            512,
            SourceRisk::Medium,
            Some("live docs".to_string()),
            "2026-03-14T00:00:01+09:00",
        )
        .expect("live open should work");
    assert_eq!(
        snapshot.source.source_url,
        format!("{}/docs", server.base_url())
    );
    assert_eq!(
        session.state.current_url,
        Some(format!("{}/docs", server.base_url()))
    );
    assert!(session
        .transcript
        .entries
        .iter()
        .any(|entry| entry.payload_type == TranscriptPayloadType::AcquisitionRecord));

    let report = runtime
        .extract(
            &mut session,
            vec![ClaimInput {
                id: "live-1".to_string(),
                statement: "The Starter plan costs $29 per month.".to_string(),
            }],
            "2026-03-14T00:00:02+09:00",
        )
        .expect("extract should work on live snapshot");
    assert_eq!(report.supported_claims.len(), 1);
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

#[test]
fn maintains_bounded_memory_across_twenty_actions() {
    let runtime = ReadOnlyRuntime::default();
    let catalog = fixture_catalog();
    let mut session = runtime.start_session("smemory001", "2026-03-14T00:00:00+09:00");
    let sequence = [
        (
            "fixture://research/static-docs/getting-started",
            "docs-1",
            "Touch Browser compiles web pages into semantic state for research agents.",
        ),
        (
            "fixture://research/citation-heavy/pricing",
            "pricing-1",
            "The Starter plan costs $29 per month.",
        ),
        (
            "fixture://research/navigation/api-reference",
            "api-1",
            "Snapshot responses include stable refs and evidence metadata.",
        ),
    ];

    let mut memory_refs = Vec::new();
    let mut memory_turns = Vec::new();

    for action_index in 0..10 {
        let step = sequence[action_index % sequence.len()];
        let open_timestamp = format!("2026-03-14T00:{:02}:00+09:00", action_index * 2);
        let extract_timestamp = format!("2026-03-14T00:{:02}:30+09:00", action_index * 2 + 1);

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
                    id: format!("{}-{action_index}", step.1),
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

    let summary = summarize_turns(&memory_turns, 12);
    let action_entries = session
        .transcript
        .entries
        .iter()
        .filter(|entry| entry.kind == TranscriptKind::Command)
        .count();

    assert_eq!(action_entries, 20);
    assert_eq!(summary.turn_count, 20);
    assert!(summary.max_working_set_size <= 6);
    assert!(summary.final_working_set_size <= 6);
    assert_eq!(summary.visited_urls.len(), 3);
    assert!(summary
        .synthesized_notes
        .iter()
        .any(|note| note == "The Starter plan costs $29 per month."));
    assert!(summary
        .synthesized_notes
        .iter()
        .any(|note| note == "Snapshot responses include stable refs and evidence metadata."));
}

#[test]
fn synthesizes_multi_page_session_reports() {
    let runtime = ReadOnlyRuntime::default();
    let catalog = fixture_catalog();
    let mut session = runtime.start_session("ssynthesis001", "2026-03-14T00:00:00+09:00");

    runtime
        .open(
            &mut session,
            &catalog,
            "fixture://research/static-docs/getting-started",
            "2026-03-14T00:00:01+09:00",
        )
        .expect("open should work");
    runtime
        .extract(
            &mut session,
            vec![ClaimInput {
                id: "claim-docs".to_string(),
                statement:
                    "Touch Browser compiles web pages into semantic state for research agents."
                        .to_string(),
            }],
            "2026-03-14T00:00:02+09:00",
        )
        .expect("extract docs should work");
    runtime
        .open(
            &mut session,
            &catalog,
            "fixture://research/citation-heavy/pricing",
            "2026-03-14T00:00:03+09:00",
        )
        .expect("pricing open should work");
    runtime
        .extract(
            &mut session,
            vec![
                ClaimInput {
                    id: "claim-pricing".to_string(),
                    statement: "The Starter plan costs $29 per month.".to_string(),
                },
                ClaimInput {
                    id: "claim-missing".to_string(),
                    statement: "The Enterprise plan starts at $9 per month.".to_string(),
                },
            ],
            "2026-03-14T00:00:04+09:00",
        )
        .expect("pricing extract should work");

    let synthesis = runtime
        .synthesize_session(&session, "2026-03-14T00:00:05+09:00", 8)
        .expect("synthesis should work");

    assert_eq!(synthesis.snapshot_count, 2);
    assert_eq!(synthesis.evidence_report_count, 2);
    assert_eq!(synthesis.visited_urls.len(), 2);
    assert!(synthesis
        .supported_claims
        .iter()
        .any(|claim| claim.claim_id == "claim-docs"));
    assert!(synthesis
        .supported_claims
        .iter()
        .any(|claim| claim.claim_id == "claim-pricing"));
    assert!(synthesis
        .unsupported_claims
        .iter()
        .chain(synthesis.needs_more_browsing_claims.iter())
        .chain(synthesis.contradicted_claims.iter())
        .any(|claim| claim.claim_id == "claim-missing"));
    assert!(synthesis
        .synthesized_notes
        .iter()
        .any(|note| note.contains("Touch Browser compiles web pages")));
    assert!(synthesis
        .synthesized_notes
        .iter()
        .any(|note| note.contains("Starter plan costs $29")));
}

#[test]
fn synthesizes_claims_by_claim_id_and_prioritizes_contradictions() {
    let runtime = ReadOnlyRuntime::default();
    let mut session = runtime.start_session("ssynthesis002", "2026-03-14T00:00:00+09:00");

    session.evidence_reports.push(EvidenceRecord {
        snapshot_id: "snapshot-1".to_string(),
        report: synthesis_report(vec![EvidenceClaimOutcome {
            version: CONTRACT_VERSION.to_string(),
            claim_id: "claim-dup".to_string(),
            statement: "Python supports async IO.".to_string(),
            verdict: EvidenceClaimVerdict::EvidenceSupported,
            support: vec!["rmain:text:async".to_string()],
            support_score: Some(0.91),
            citation: Some(sample_citation("https://example.com/async")),
            reason: None,
            checked_block_refs: vec!["rmain:text:async".to_string()],
            guard_failures: Vec::new(),
            next_action_hint: None,
            verification_verdict: None,
        }]),
    });
    session.evidence_reports.push(EvidenceRecord {
        snapshot_id: "snapshot-2".to_string(),
        report: synthesis_report(vec![EvidenceClaimOutcome {
            version: CONTRACT_VERSION.to_string(),
            claim_id: "claim-dup".to_string(),
            statement: "Python supports async IO".to_string(),
            verdict: EvidenceClaimVerdict::Contradicted,
            support: Vec::new(),
            support_score: None,
            citation: None,
            reason: Some(UnsupportedClaimReason::ContradictoryEvidence),
            checked_block_refs: vec!["rmain:text:sync-only".to_string()],
            guard_failures: Vec::new(),
            next_action_hint: None,
            verification_verdict: None,
        }]),
    });

    let synthesis = runtime
        .synthesize_session(&session, "2026-03-14T00:00:05+09:00", 8)
        .expect("synthesis should work");

    assert!(synthesis.supported_claims.is_empty());
    assert_eq!(synthesis.contradicted_claims.len(), 1);
    assert_eq!(synthesis.contradicted_claims[0].claim_id, "claim-dup");
    assert_eq!(
        synthesis.contradicted_claims[0].statement,
        "Python supports async IO."
    );
    assert_eq!(
        synthesis.contradicted_claims[0].snapshot_ids,
        vec!["snapshot-1".to_string(), "snapshot-2".to_string()]
    );
}

fn synthesis_report(claim_outcomes: Vec<EvidenceClaimOutcome>) -> EvidenceReport {
    let mut report = EvidenceReport {
        version: CONTRACT_VERSION.to_string(),
        generated_at: "2026-03-14T00:00:00+09:00".to_string(),
        source: EvidenceSource {
            source_url: "https://example.com".to_string(),
            source_type: SourceType::Http,
            source_risk: SourceRisk::Low,
            source_label: Some("Example".to_string()),
        },
        supported_claims: Vec::new(),
        contradicted_claims: Vec::new(),
        unsupported_claims: Vec::new(),
        needs_more_browsing_claims: Vec::new(),
        claim_outcomes,
        verification: None,
    };
    report.rebuild_claim_buckets();
    report
}

fn sample_citation(url: &str) -> EvidenceCitation {
    EvidenceCitation {
        url: url.to_string(),
        retrieved_at: "2026-03-14T00:00:00+09:00".to_string(),
        source_type: SourceType::Http,
        source_risk: SourceRisk::Low,
        source_label: Some("Example".to_string()),
    }
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

struct LiveTestServer {
    base_url: String,
    stop_flag: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl LiveTestServer {
    fn start(requests: Arc<AtomicUsize>) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("listener bind");
        let address = listener.local_addr().expect("local addr");
        let server = Server::from_listener(listener, None).expect("server");
        let stop_flag = Arc::new(AtomicBool::new(false));
        let stop_flag_thread = stop_flag.clone();
        let base_url = format!("http://{}", address);

        let handle = thread::spawn(move || {
            while !stop_flag_thread.load(Ordering::SeqCst) {
                let Ok(Some(request)) = server.recv_timeout(std::time::Duration::from_millis(100))
                else {
                    continue;
                };
                requests.fetch_add(1, Ordering::SeqCst);
                let path = request.url().to_string();

                let response = match path.as_str() {
                    "/robots.txt" => text_response("User-agent: *\nDisallow:\n", 200),
                    "/start" => redirect_response("/docs"),
                    "/docs" => html_response(
                        "<html><head><title>Live Docs</title></head><body><main><h1>Pricing</h1><p>The Starter plan costs $29 per month.</p></main></body></html>",
                        "text/html; charset=utf-8",
                        200,
                    ),
                    _ => html_response("<html><body>missing</body></html>", "text/html", 404),
                };

                let _ = request.respond(response);
            }
        });

        Self {
            base_url,
            stop_flag,
            handle: Some(handle),
        }
    }

    fn base_url(&self) -> &str {
        &self.base_url
    }
}

impl Drop for LiveTestServer {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

fn html_response(body: &str, content_type: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Content-Type", content_type).expect("header");
    TinyResponse::new(
        StatusCode(status),
        vec![header],
        Cursor::new(body.as_bytes().to_vec()),
        Some(body.len()),
        None,
    )
}

fn text_response(body: &str, status: u16) -> TinyResponse<Cursor<Vec<u8>>> {
    html_response(body, "text/plain; charset=utf-8", status)
}

fn redirect_response(location: &str) -> TinyResponse<Cursor<Vec<u8>>> {
    let header = Header::from_bytes("Location", location).expect("location");
    TinyResponse::new(
        StatusCode(302),
        vec![header],
        Cursor::new(Vec::new()),
        Some(0),
        None,
    )
}
