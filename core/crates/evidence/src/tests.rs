use std::{collections::BTreeMap, fs, path::PathBuf};

use serde::Deserialize;
use serde_json::{json, Value};
use touch_browser_contracts::{
    EvidenceClaimVerdict, EvidenceReport, SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole,
    SnapshotBudget, SnapshotDocument, SnapshotEvidence, SnapshotSource, SourceRisk, SourceType,
    UnsupportedClaimReason, CONTRACT_VERSION, STABLE_REF_VERSION,
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

fn snapshot_source(url: &str, source_type: SourceType, title: &str) -> SnapshotSource {
    SnapshotSource {
        source_url: url.to_string(),
        source_type,
        title: Some(title.to_string()),
    }
}

fn snapshot_budget(requested_tokens: usize, estimated_tokens: usize) -> SnapshotBudget {
    SnapshotBudget {
        requested_tokens,
        estimated_tokens,
        emitted_tokens: estimated_tokens,
        truncated: false,
    }
}

fn snapshot_evidence(url: &str, source_type: SourceType, dom_path_hint: &str) -> SnapshotEvidence {
    SnapshotEvidence {
        source_url: url.to_string(),
        source_type,
        dom_path_hint: Some(dom_path_hint.to_string()),
        byte_range_start: None,
        byte_range_end: None,
    }
}

#[allow(clippy::too_many_arguments)]
fn content_block(
    url: &str,
    source_type: SourceType,
    id: &str,
    kind: SnapshotBlockKind,
    stable_ref: &str,
    role: SnapshotBlockRole,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotBlock {
    SnapshotBlock {
        version: CONTRACT_VERSION.to_string(),
        id: id.to_string(),
        kind,
        stable_ref: stable_ref.to_string(),
        role,
        text: text.to_string(),
        attributes: Default::default(),
        evidence: snapshot_evidence(url, source_type, dom_path_hint),
    }
}

#[allow(clippy::too_many_arguments)]
fn content_block_with_attributes(
    url: &str,
    source_type: SourceType,
    id: &str,
    kind: SnapshotBlockKind,
    stable_ref: &str,
    role: SnapshotBlockRole,
    text: &str,
    dom_path_hint: &str,
    attributes: BTreeMap<String, Value>,
) -> SnapshotBlock {
    SnapshotBlock {
        attributes,
        ..content_block(
            url,
            source_type,
            id,
            kind,
            stable_ref,
            role,
            text,
            dom_path_hint,
        )
    }
}

fn text_block(
    url: &str,
    source_type: SourceType,
    id: &str,
    stable_ref: &str,
    role: SnapshotBlockRole,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotBlock {
    content_block(
        url,
        source_type,
        id,
        SnapshotBlockKind::Text,
        stable_ref,
        role,
        text,
        dom_path_hint,
    )
}

fn heading_block(
    url: &str,
    source_type: SourceType,
    id: &str,
    stable_ref: &str,
    text: &str,
    dom_path_hint: &str,
    level: u64,
) -> SnapshotBlock {
    content_block_with_attributes(
        url,
        source_type,
        id,
        SnapshotBlockKind::Heading,
        stable_ref,
        SnapshotBlockRole::Content,
        text,
        dom_path_hint,
        attrs(json!({ "level": level })),
    )
}

fn list_block(
    url: &str,
    source_type: SourceType,
    id: &str,
    stable_ref: &str,
    role: SnapshotBlockRole,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotBlock {
    content_block(
        url,
        source_type,
        id,
        SnapshotBlockKind::List,
        stable_ref,
        role,
        text,
        dom_path_hint,
    )
}

fn button_block(
    url: &str,
    source_type: SourceType,
    id: &str,
    stable_ref: &str,
    role: SnapshotBlockRole,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotBlock {
    content_block(
        url,
        source_type,
        id,
        SnapshotBlockKind::Button,
        stable_ref,
        role,
        text,
        dom_path_hint,
    )
}

fn link_block(
    url: &str,
    source_type: SourceType,
    id: &str,
    stable_ref: &str,
    role: SnapshotBlockRole,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotBlock {
    content_block(
        url,
        source_type,
        id,
        SnapshotBlockKind::Link,
        stable_ref,
        role,
        text,
        dom_path_hint,
    )
}

fn single_text_snapshot(
    url: &str,
    source_type: SourceType,
    title: &str,
    stable_ref: &str,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotDocument {
    snapshot_document(
        url,
        source_type.clone(),
        title,
        512,
        32,
        vec![text_block(
            url,
            source_type,
            "b1",
            stable_ref,
            SnapshotBlockRole::Content,
            text,
            dom_path_hint,
        )],
    )
}

fn snapshot_document(
    url: &str,
    source_type: SourceType,
    title: &str,
    requested_tokens: usize,
    estimated_tokens: usize,
    blocks: Vec<SnapshotBlock>,
) -> SnapshotDocument {
    SnapshotDocument {
        version: CONTRACT_VERSION.to_string(),
        stable_ref_version: STABLE_REF_VERSION.to_string(),
        source: snapshot_source(url, source_type, title),
        budget: snapshot_budget(requested_tokens, estimated_tokens),
        blocks,
    }
}

fn nodejs_about_snapshot(standard_library_role: SnapshotBlockRole) -> SnapshotDocument {
    snapshot_document(
        "https://nodejs.org/en/about",
        SourceType::Playwright,
        "About Node.js",
        512,
        96,
        vec![
            heading_block(
                "https://nodejs.org/en/about",
                SourceType::Playwright,
                "b1",
                "rmain:heading:about-nodejs",
                "About Node.js",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "https://nodejs.org/en/about",
                SourceType::Playwright,
                "b2",
                "rmain:text:runtime-overview",
                SnapshotBlockRole::Content,
                "As an asynchronous event-driven JavaScript runtime, Node.js is designed to build scalable network applications.",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "https://nodejs.org/en/about",
                SourceType::Playwright,
                "b3",
                "rmain:text:standard-library",
                standard_library_role,
                "The synchronous methods of the Node.js standard library are convenient for startup tasks.",
                "html > body > main > p:nth-of-type(2)",
            ),
        ],
    )
}

fn nodejs_blocking_overview_snapshot() -> SnapshotDocument {
    snapshot_document(
        "https://nodejs.org/en/learn/asynchronous-work/overview-of-blocking-vs-non-blocking",
        SourceType::Http,
        "Overview of Blocking vs Non-Blocking",
        512,
        96,
        vec![
            heading_block(
                "https://nodejs.org/en/learn/asynchronous-work/overview-of-blocking-vs-non-blocking",
                SourceType::Http,
                "b1",
                "rmain:heading:blocking-vs-non-blocking",
                "Overview of Blocking vs Non-Blocking",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "https://nodejs.org/en/learn/asynchronous-work/overview-of-blocking-vs-non-blocking",
                SourceType::Http,
                "b92",
                "rmain:text:blocking-is-when-the-execution-of-additional-jav",
                SnapshotBlockRole::Content,
                "Blocking is when the execution of additional JavaScript in the Node.js process must wait until a non-JavaScript operation completes.",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "https://nodejs.org/en/learn/asynchronous-work/overview-of-blocking-vs-non-blocking",
                SourceType::Http,
                "b93",
                "rmain:text:in-node-js-javascript-that-exhibits-poor-perform",
                SnapshotBlockRole::Content,
                "Synchronous methods in the Node.js standard library that use libuv are the most commonly used blocking operations.",
                "html > body > main > p:nth-of-type(2)",
            ),
            text_block(
                "https://nodejs.org/en/learn/asynchronous-work/overview-of-blocking-vs-non-blocking",
                SourceType::Http,
                "b94",
                "rmain:text:all-of-the-i-o-methods-in-the-node-js-standard-l",
                SnapshotBlockRole::Content,
                "All of the I/O methods in the Node.js standard library provide asynchronous versions, which are non-blocking, and accept callback functions.",
                "html > body > main > p:nth-of-type(3)",
            ),
        ],
    )
}

fn live_like_python_snapshot() -> SnapshotDocument {
    snapshot_document(
        "https://ko.wikipedia.org/wiki/%ED%8C%8C%EC%9D%B4%EC%8D%AC",
        SourceType::Http,
        "파이썬",
        512,
        128,
        vec![
            text_block(
                "https://ko.wikipedia.org/wiki/%ED%8C%8C%EC%9D%B4%EC%8D%AC",
                SourceType::Http,
                "b239",
                "rmain:text:mwcw",
                SnapshotBlockRole::Content,
                "파이썬은 1991년 발표된 고급 프로그래밍 언어로, 인터프리터를 사용하는 객체지향 언어이자 플랫폼에 독립적인 동적 타이핑 대화형 언어다.",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "https://ko.wikipedia.org/wiki/%ED%8C%8C%EC%9D%B4%EC%8D%AC",
                SourceType::Http,
                "b246",
                "rmain:text:mwvg",
                SnapshotBlockRole::Content,
                "현대의 파이썬은 여전히 인터프리터 언어처럼 동작하나 사용자가 모르는 사이에 스스로 파이썬 소스 코드를 컴파일하여 바이트 코드를 만들어 낸다.",
                "html > body > main > p:nth-of-type(2)",
            ),
            text_block(
                "https://ko.wikipedia.org/wiki/%ED%8C%8C%EC%9D%B4%EC%8D%AC",
                SourceType::Http,
                "b261",
                "rmain:text:mwkq",
                SnapshotBlockRole::Supporting,
                "파이썬은 다양한 프로그래밍 패러다임을 지원하는 언어이다.",
                "html > body > main > p:nth-of-type(3)",
            ),
            text_block(
                "https://ko.wikipedia.org/wiki/%ED%8C%8C%EC%9D%B4%EC%8D%AC",
                SourceType::Http,
                "b282",
                "rmain:text:mwaxy",
                SnapshotBlockRole::Supporting,
                "파이썬은 동적 타이핑을 사용하는 언어다.",
                "html > body > main > p:nth-of-type(4)",
            ),
        ],
    )
}

fn extract_report(
    snapshot: SnapshotDocument,
    claims: Vec<ClaimRequest>,
    timestamp: &str,
    risk: SourceRisk,
    title: impl Into<Option<String>>,
) -> EvidenceReport {
    EvidenceExtractor
        .extract(&EvidenceInput::new(
            snapshot,
            claims,
            timestamp,
            risk,
            title.into(),
        ))
        .expect("evidence extraction should succeed")
}

fn claim(id: &str, statement: impl Into<String>) -> ClaimRequest {
    ClaimRequest::new(id, statement)
}

fn attrs(value: Value) -> BTreeMap<String, Value> {
    value
        .as_object()
        .expect("attributes should be an object")
        .clone()
        .into_iter()
        .collect()
}

fn assert_supported_only(report: &EvidenceReport) {
    assert_eq!(report.supported_claims.len(), 1);
    assert!(report.contradicted_claims.is_empty());
    assert!(report.needs_more_browsing_claims.is_empty());
    assert!(report.unsupported_claims.is_empty());
}

fn assert_contradicted_only(report: &EvidenceReport, expected_reason: UnsupportedClaimReason) {
    assert!(report.supported_claims.is_empty());
    assert_eq!(report.contradicted_claims.len(), 1);
    assert_eq!(report.contradicted_claims[0].reason, expected_reason);
}

fn assert_needs_more_browsing_only(
    report: &EvidenceReport,
    expected_reason: UnsupportedClaimReason,
) {
    assert!(report.supported_claims.is_empty());
    assert!(report.contradicted_claims.is_empty());
    assert_eq!(report.needs_more_browsing_claims.len(), 1);
    assert_eq!(report.needs_more_browsing_claims[0].reason, expected_reason);
}

fn assert_unsupported_only(report: &EvidenceReport, expected_reason: UnsupportedClaimReason) {
    assert!(report.supported_claims.is_empty());
    assert!(report.contradicted_claims.is_empty());
    assert!(report.needs_more_browsing_claims.is_empty());
    assert_eq!(report.unsupported_claims.len(), 1);
    assert_eq!(report.unsupported_claims[0].reason, expected_reason);
}

fn moon_landing_source() -> SnapshotSource {
    SnapshotSource {
        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
        source_type: SourceType::Http,
        title: Some("Moon landing".to_string()),
    }
}

fn moon_landing_budget() -> SnapshotBudget {
    SnapshotBudget {
        requested_tokens: 1024,
        estimated_tokens: 256,
        emitted_tokens: 256,
        truncated: false,
    }
}

fn moon_landing_evidence(dom_path_hint: &str) -> SnapshotEvidence {
    SnapshotEvidence {
        source_url: "https://en.wikipedia.org/wiki/Moon_landing".to_string(),
        source_type: SourceType::Http,
        dom_path_hint: Some(dom_path_hint.to_string()),
        byte_range_start: None,
        byte_range_end: None,
    }
}

fn moon_landing_block(
    id: &str,
    kind: SnapshotBlockKind,
    stable_ref: &str,
    text: &str,
    dom_path_hint: &str,
) -> SnapshotBlock {
    SnapshotBlock {
        version: "1.0.0".to_string(),
        id: id.to_string(),
        kind,
        stable_ref: stable_ref.to_string(),
        role: SnapshotBlockRole::Content,
        text: text.to_string(),
        attributes: Default::default(),
        evidence: moon_landing_evidence(dom_path_hint),
    }
}

fn moon_landing_table_noise_text() -> &'static str {
    "Mission | Mass kg | Booster | Launch date | Goal | Result | Luna 1 | 12 | 20 | 5 | 727 | 14 | 1972 | 0 | 05 | Apollo 11 | 003 | 056 | 8 | 17 | 2221 | 1966 | 1976 | 2 | 41 | 292 | 30 | 000 | 1480 | 1968 | 39 | 3 | 15"
}

fn moon_landing_table_noise_block(id: &str) -> SnapshotBlock {
    moon_landing_block(
        id,
        SnapshotBlockKind::Table,
        "rmain:table:mission-mass-kg-booster-launch-date-goal-result-",
        moon_landing_table_noise_text(),
        "html > body > main > table",
    )
}

fn moon_landing_snapshot(blocks: Vec<SnapshotBlock>) -> SnapshotDocument {
    SnapshotDocument {
        version: "1.0.0".to_string(),
        stable_ref_version: "1".to_string(),
        source: moon_landing_source(),
        budget: moon_landing_budget(),
        blocks,
    }
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

    assert_unsupported_only(&report, UnsupportedClaimReason::InsufficientConfidence);
}

#[test]
fn marks_contradictory_claim_as_unsupported() {
    let snapshot = snapshot_document(
        "https://www.iana.org/help/example-domains",
        SourceType::Playwright,
        "Example Domains",
        512,
        32,
        vec![text_block(
            "https://www.iana.org/help/example-domains",
            SourceType::Playwright,
            "b1",
            "rmain:text:example-domains-note",
            SnapshotBlockRole::Content,
            "example.com is not available for registration or transfer. These domains are available for documentation examples.",
            "html > body > main",
        )],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "example.com is available for registration or transfer.",
        )],
        "2026-03-17T00:00:00+09:00",
        SourceRisk::Low,
        Some("Example Domains".to_string()),
    );

    assert_contradicted_only(&report, UnsupportedClaimReason::NegationMismatch);
}

#[test]
fn supports_negative_availability_claim_when_page_matches_negative_polarity() {
    let snapshot = snapshot_document(
        "https://www.iana.org/help/example-domains",
        SourceType::Playwright,
        "Example Domains",
        512,
        32,
        vec![text_block(
            "https://www.iana.org/help/example-domains",
            SourceType::Playwright,
            "b1",
            "rmain:text:example-domains-note",
            SnapshotBlockRole::Content,
            "example.com is not available for registration or transfer. These domains are available for documentation examples.",
            "html > body > main",
        )],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "example.com is not available for registration or transfer.",
        )],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("Example Domains".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn does_not_treat_plural_notes_as_negation() {
    let snapshot = snapshot_document(
        "fixture://research/navigation/browser-expand",
        SourceType::Playwright,
        "Browser Expand",
        512,
        32,
        vec![text_block(
            "fixture://research/navigation/browser-expand",
            SourceType::Playwright,
            "b1",
            "rmain:text:details",
            SnapshotBlockRole::Content,
            "Expanded details confirm that the runtime can reveal collapsed notes.",
            "html > body > main",
        )],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "Expanded details confirm that the runtime can reveal collapsed notes.",
        )],
        "2026-03-17T00:00:00+09:00",
        SourceRisk::Low,
        Some("Browser Expand".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn rejects_plausible_claims_when_anchor_or_qualifier_coverage_is_missing() {
    let snapshot = snapshot_document(
        "https://docs.aws.example/ecs",
        SourceType::Http,
        "Example ECS Overview",
        512,
        64,
        vec![
            text_block(
                "https://docs.aws.example/ecs",
                SourceType::Http,
                "b1",
                "rmain:text:overview",
                SnapshotBlockRole::Content,
                "Amazon ECS is a fully managed container orchestration service.",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "https://docs.aws.example/ecs",
                SourceType::Http,
                "b2",
                "rmain:text:managed-instances",
                SnapshotBlockRole::Content,
                "Managed instances support GPU acceleration for selected workloads.",
                "html > body > main > p:nth-of-type(2)",
            ),
            text_block(
                "https://docs.aws.example/ecs",
                SourceType::Http,
                "b3",
                "rmain:text:regional",
                SnapshotBlockRole::Content,
                "Availability varies by Region and capacity option.",
                "html > body > main > p:nth-of-type(3)",
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![
            claim("c1", "ECS supports GPU instances natively."),
            claim("c2", "ECS is available in all AWS regions."),
        ],
        "2026-04-05T00:00:00+09:00",
        SourceRisk::Low,
        Some("Example ECS Overview".to_string()),
    );

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
    let snapshot = snapshot_document(
        "https://docs.aws.example/ecs/welcome",
        SourceType::Http,
        "What is Amazon Elastic Container Service?",
        1024,
        128,
        vec![
            heading_block(
                "https://docs.aws.example/ecs/welcome",
                SourceType::Http,
                "b1",
                "rmain:heading:welcome",
                "What is Amazon Elastic Container Service?",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "https://docs.aws.example/ecs/welcome",
                SourceType::Http,
                "b2",
                "rmain:text:controller",
                SnapshotBlockRole::Content,
                "Controller - Deploy and manage your applications that run on the containers.",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "https://docs.aws.example/ecs/welcome",
                SourceType::Http,
                "b3",
                "rmain:text:managed",
                SnapshotBlockRole::Content,
                "Amazon ECS Managed Instances offloads infrastructure management to AWS for containerized workloads.",
                "html > body > main > p:nth-of-type(2)",
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "Amazon ECS is a fully managed container orchestration service.",
        )],
        "2026-04-05T00:00:00+09:00",
        SourceRisk::Low,
        Some("What is Amazon Elastic Container Service?".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn does_not_promote_interaction_claims_from_single_button_context() {
    let snapshot = snapshot_document(
        "fixture://research/navigation/browser-pagination",
        SourceType::Fixture,
        "Browser Pagination",
        512,
        32,
        vec![
            heading_block(
                "fixture://research/navigation/browser-pagination",
                SourceType::Fixture,
                "b1",
                "rmain:heading:browser-pagination",
                "Browser Pagination",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "fixture://research/navigation/browser-pagination",
                SourceType::Fixture,
                "b2",
                "rmain:text:page-label",
                SnapshotBlockRole::Content,
                "Page 1",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "fixture://research/navigation/browser-pagination",
                SourceType::Fixture,
                "b3",
                "rmain:text:page-content",
                SnapshotBlockRole::Content,
                "Page 1 collects the first batch of release highlights.",
                "html > body > main > p:nth-of-type(2)",
            ),
            button_block(
                "fixture://research/navigation/browser-pagination",
                SourceType::Fixture,
                "b4",
                "rmain:button:next",
                SnapshotBlockRole::FormControl,
                "Next",
                "html > body > main > button",
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![claim("c1", "Page 1 includes a Next button.")],
        "2026-04-05T00:00:00+09:00",
        SourceRisk::Low,
        Some("Browser Pagination".to_string()),
    );

    assert_needs_more_browsing_only(&report, UnsupportedClaimReason::NeedsMoreBrowsing);
}

#[test]
fn rejects_numeric_mismatches_as_contradicted() {
    let snapshot = snapshot_document(
        "https://docs.aws.example/lambda/limits",
        SourceType::Http,
        "Lambda quotas",
        512,
        64,
        vec![
            heading_block(
                "https://docs.aws.example/lambda/limits",
                SourceType::Http,
                "b1",
                "rmain:heading:function-configuration",
                "Function configuration, deployment, and execution",
                "html > body > main > h2",
                2,
            ),
            text_block(
                "https://docs.aws.example/lambda/limits",
                SourceType::Http,
                "b2",
                "rmain:text:timeout",
                SnapshotBlockRole::Content,
                "Function timeout: 900 seconds (15 minutes).",
                "html > body > main > p:nth-of-type(1)",
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "The maximum timeout for a Lambda function is 24 hours.",
        )],
        "2026-04-05T00:00:00+09:00",
        SourceRisk::Low,
        Some("Lambda quotas".to_string()),
    );

    assert_contradicted_only(&report, UnsupportedClaimReason::NumericMismatch);
}

#[test]
fn defers_table_numeric_noise_to_needs_more_browsing() {
    let snapshot = moon_landing_snapshot(vec![
        moon_landing_block(
            "b1",
            SnapshotBlockKind::Heading,
            "rmain:heading:soviet-uncrewed-soft-landings-1966-1976",
            "Soviet uncrewed soft landings (1966-1976)",
            "html > body > main > h2",
        ),
        moon_landing_table_noise_block("b2"),
        moon_landing_block(
            "b3",
            SnapshotBlockKind::List,
            "rmain:list:mission-mass-kg-booster-launch-date-goal-result-",
            "- Apollo 11 - 20 - 1968 - crewed mission - Moon landing - lunar surface operations",
            "html > body > main > ul",
        ),
    ]);

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

    assert_needs_more_browsing_only(&report, UnsupportedClaimReason::NeedsMoreBrowsing);
}

#[test]
fn prefers_narrative_support_over_numeric_table_noise_for_date_claims() {
    let snapshot = moon_landing_snapshot(vec![
        moon_landing_table_noise_block("b1"),
        moon_landing_block(
            "b2",
            SnapshotBlockKind::Text,
            "rmain:text:apollo-11-first-crewed-moon-landing",
            "Apollo 11 was the first crewed Moon landing on July 20, 1969.",
            "html > body > main > p:nth-of-type(1)",
        ),
        moon_landing_block(
            "b3",
            SnapshotBlockKind::Text,
            "rmain:text:apollo-11-human-landing",
            "The mission marked humanity's first landing on the Moon.",
            "html > body > main > p:nth-of-type(2)",
        ),
    ]);

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

    assert_supported_only(&report);
    assert!(
        report.supported_claims[0]
            .support
            .contains(&"b2".to_string()),
        "expected narrative text block to be part of the selected support"
    );
}

#[test]
fn supports_cjk_claims_when_main_subject_terms_are_present() {
    let snapshot = single_text_snapshot(
        "https://ko.wikipedia.example/wiki/Python",
        SourceType::Http,
        "Python",
        "rmain:text:python-origin",
        "파이썬은 1991년 귀도 반 로섬이 발표한 프로그래밍 언어이다.",
        "html > body > main > p:nth-of-type(1)",
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "파이썬은 1991년 귀도 반 로섬이 발표한 프로그래밍 언어이다.",
        )],
        "2026-04-05T00:00:00+09:00",
        SourceRisk::Low,
        Some("Python".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn supports_japanese_claims_when_main_subject_terms_are_present() {
    let snapshot = single_text_snapshot(
        "https://ja.wikipedia.example/wiki/明治維新",
        SourceType::Http,
        "明治維新",
        "rmain:text:meiji",
        "明治維新は江戸幕府に対する倒幕運動から始まった日本の近代化改革である。",
        "html > body > main > p:nth-of-type(1)",
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "明治維新は江戸幕府に対する倒幕運動から始まった日本の近代化改革である。",
        )],
        "2026-04-05T00:00:00+09:00",
        SourceRisk::Low,
        Some("明治維新".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn supports_simplified_chinese_claims_against_traditional_snapshot_text() {
    let snapshot = single_text_snapshot(
        "https://zh.wikipedia.example/wiki/%E4%B8%AD%E5%9B%BD",
        SourceType::Http,
        "中國",
        "rmain:text:china-overview",
        "中國是以漢族為主體民族的國家。",
        "html > body > main > p:nth-of-type(1)",
    );

    let report = extract_report(
        snapshot,
        vec![claim("c1", "中国是以汉族为主体民族的国家。")],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("中國".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn supports_paraphrased_claims_from_adjacent_evidence_blocks() {
    let snapshot = snapshot_document(
        "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API",
        SourceType::Http,
        "Fetch API",
        512,
        96,
        vec![
            heading_block(
                "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API",
                SourceType::Http,
                "b1",
                "rmain:heading:fetch-api",
                "Fetch API",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API",
                SourceType::Http,
                "b2",
                "rmain:text:interface",
                SnapshotBlockRole::Content,
                "The Fetch API provides an interface for fetching resources.",
                "html > body > main > p:nth-of-type(1)",
            ),
            text_block(
                "https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API",
                SourceType::Http,
                "b3",
                "rmain:text:promise",
                SnapshotBlockRole::Content,
                "The fetch() method returns a Promise that resolves to the Response to that request.",
                "html > body > main > p:nth-of-type(2)",
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "The Fetch API lets JavaScript code request resources and returns a promise-based response model.",
        )],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("Fetch API".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn prefers_main_content_over_navigation_for_js_docs_claims() {
    let snapshot = snapshot_document(
        "https://reactrouter.com/home",
        SourceType::Playwright,
        "React Router Home",
        512,
        128,
        vec![
            link_block(
                "https://reactrouter.com/home",
                SourceType::Playwright,
                "b1",
                "rnav:link:framework-conventions",
                SnapshotBlockRole::PrimaryNav,
                "API Framework Conventions",
                "html > body > nav > a:nth-of-type(1)",
            ),
            heading_block(
                "https://reactrouter.com/home",
                SourceType::Playwright,
                "b2",
                "rmain:heading:react-router-home",
                "React Router Home",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "https://reactrouter.com/home",
                SourceType::Playwright,
                "b3",
                "rmain:text:intro",
                SnapshotBlockRole::Content,
                "React Router is a multi-strategy router for React bridging the gap from React 18 to React 19.",
                "html > body > main > p:nth-of-type(1)",
            ),
            list_block(
                "https://reactrouter.com/home",
                SourceType::Playwright,
                "b4",
                "rmain:list:modes",
                SnapshotBlockRole::Content,
                "- Framework - Data - Declarative",
                "html > body > main > ul",
            ),
            text_block(
                "https://reactrouter.com/home",
                SourceType::Playwright,
                "b5",
                "rmain:text:modes-explainer",
                SnapshotBlockRole::Content,
                "These icons indicate which mode the content is relevant to.",
                "html > body > main > p:nth-of-type(2)",
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "React Router supports both declarative routing and framework-style features for modern React apps.",
        )],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("React Router Home".to_string()),
    );

    assert_supported_only(&report);
    assert!(report.claim_outcomes[0]
        .checked_block_refs
        .iter()
        .all(|reference| !reference.starts_with("rnav:")));
}

#[test]
fn rejects_synchronous_runtime_claim_when_support_is_asynchronous() {
    let snapshot = nodejs_about_snapshot(SnapshotBlockRole::Content);

    let report = extract_report(
        snapshot,
        vec![claim("c1", "Node.js is a synchronous runtime.")],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("About Node.js".to_string()),
    );

    assert!(report.supported_claims.is_empty());
    assert!(report
        .contradicted_claims
        .iter()
        .any(|claim| claim.claim_id == "c1"));
}

#[test]
fn rejects_compiled_language_claim_on_live_like_python_snapshot() {
    let snapshot = live_like_python_snapshot();

    let report = extract_report(
        snapshot,
        vec![claim("c1", "파이썬은 컴파일 언어이다.")],
        "2026-04-10T00:00:00+09:00",
        SourceRisk::Low,
        Some("파이썬".to_string()),
    );

    assert!(report.supported_claims.is_empty());
    assert!(matches!(
        report.claim_outcomes[0].verdict,
        EvidenceClaimVerdict::Contradicted | EvidenceClaimVerdict::NeedsMoreBrowsing
    ));
    assert!(
        report.claim_outcomes[0]
            .guard_failures
            .iter()
            .any(|failure| failure.kind == touch_browser_contracts::EvidenceGuardKind::Predicate)
            || report.claim_outcomes[0].reason == Some(UnsupportedClaimReason::PredicateMismatch)
    );
}

#[test]
fn supports_interpreted_language_claim_on_live_like_python_snapshot() {
    let snapshot = live_like_python_snapshot();

    let report = extract_report(
        snapshot,
        vec![claim("c1", "파이썬은 인터프리터 언어이다.")],
        "2026-04-10T00:00:00+09:00",
        SourceRisk::Low,
        Some("파이썬".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn supports_asynchronous_runtime_claim_when_synchronous_is_only_contrastive() {
    let snapshot = nodejs_about_snapshot(SnapshotBlockRole::Supporting);

    let report = extract_report(
        snapshot,
        vec![claim("c1", "Node.js is an asynchronous runtime.")],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("About Node.js".to_string()),
    );

    assert_supported_only(&report);
}

#[test]
fn requires_more_browsing_for_bare_runtime_behavior_claim_on_method_level_page() {
    let snapshot = nodejs_blocking_overview_snapshot();

    let sync_report = extract_report(
        snapshot.clone(),
        vec![claim("c1", "Node.js is synchronous.")],
        "2026-04-10T00:00:00+09:00",
        SourceRisk::Low,
        Some("Overview of Blocking vs Non-Blocking".to_string()),
    );

    assert_eq!(
        sync_report.claim_outcomes[0].verdict,
        EvidenceClaimVerdict::NeedsMoreBrowsing
    );

    let async_report = extract_report(
        snapshot,
        vec![claim("c2", "Node.js is asynchronous.")],
        "2026-04-10T00:00:00+09:00",
        SourceRisk::Low,
        Some("Overview of Blocking vs Non-Blocking".to_string()),
    );

    assert_eq!(
        async_report.claim_outcomes[0].verdict,
        EvidenceClaimVerdict::NeedsMoreBrowsing
    );
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
    let snapshot = snapshot_document(
        "https://nodejs.org/en/download",
        SourceType::Playwright,
        "Node.js Downloads",
        512,
        96,
        vec![
            heading_block(
                "https://nodejs.org/en/download",
                SourceType::Playwright,
                "b1",
                "rmain:heading:downloads",
                "Node.js Downloads",
                "html > body > main > h1",
                1,
            ),
            text_block(
                "https://nodejs.org/en/download",
                SourceType::Playwright,
                "b2",
                "rmain:text:download-selector",
                SnapshotBlockRole::Content,
                "Choose a platform to download Node.js installers.",
                "html > body > main > p:nth-of-type(1)",
            ),
            content_block_with_attributes(
                "https://nodejs.org/en/download",
                SourceType::Playwright,
                "b3",
                SnapshotBlockKind::List,
                "rmain:list:platform-options",
                SnapshotBlockRole::Supporting,
                "- macOS - Windows - Linux",
                "html > body > main > ul[role=listbox]",
                attrs(json!({
                    "zone": "main",
                    "tagName": "listbox",
                    "options": ["macOS", "Windows", "Linux"],
                    "selectionSemantic": "available-options",
                    "textLength": 27
                })),
            ),
        ],
    );

    let report = extract_report(
        snapshot,
        vec![claim("c1", "Node.js is available for macOS.")],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("Node.js Downloads".to_string()),
    );

    assert_eq!(report.supported_claims.len(), 1);
    assert_eq!(report.supported_claims[0].claim_id, "c1");
}

#[test]
fn rejects_low_signal_repetitive_claims() {
    let repetitive_claim = format!("파이썬은 {}좋은 언어이다", "매우 ".repeat(200));
    let snapshot = single_text_snapshot(
        "https://www.python.org/",
        SourceType::Http,
        "Welcome to Python.org",
        "rmain:text:intro",
        "Python is a powerful programming language.",
        "html > body > main > p",
    );

    let report = extract_report(
        snapshot,
        vec![claim("c1", repetitive_claim)],
        "2026-04-07T00:00:00+09:00",
        SourceRisk::Low,
        Some("Welcome to Python.org".to_string()),
    );

    assert_unsupported_only(&report, UnsupportedClaimReason::InsufficientConfidence);
}

#[test]
fn rejects_default_timeout_claim_when_support_only_states_maximum_timeout() {
    let snapshot = single_text_snapshot(
        "https://docs.aws.example/lambda/limits",
        SourceType::Http,
        "Lambda quotas",
        "rmain:text:timeout",
        "The maximum timeout for a Lambda function is 900 seconds (15 minutes).",
        "html > body > main > p:nth-of-type(1)",
    );

    let report = extract_report(
        snapshot,
        vec![claim(
            "c1",
            "The default timeout for a Lambda function is 15 minutes.",
        )],
        "2026-04-09T00:00:00+09:00",
        SourceRisk::Low,
        Some("Lambda quotas".to_string()),
    );

    assert_needs_more_browsing_only(&report, UnsupportedClaimReason::NeedsMoreBrowsing);
}

#[test]
fn distinguishes_default_and_maximum_claims_inside_the_same_block() {
    let snapshot = snapshot_document(
        "https://docs.aws.example/lambda/limits",
        SourceType::Http,
        "Lambda quotas",
        512,
        48,
        vec![text_block(
            "https://docs.aws.example/lambda/limits",
            SourceType::Http,
            "b1",
            "rmain:text:timeout",
            SnapshotBlockRole::Content,
            "The default timeout for a Lambda function is 3 seconds, but you can adjust the Lambda function timeout in increments of 1 second up to a maximum timeout of 900 seconds (15 minutes).",
            "html > body > main > p:nth-of-type(1)",
        )],
    );

    let report = extract_report(
        snapshot.clone(),
        vec![claim(
            "c1",
            "The default timeout for a Lambda function is 15 minutes.",
        )],
        "2026-04-09T00:00:00+09:00",
        SourceRisk::Low,
        Some("Lambda quotas".to_string()),
    );

    assert!(report.supported_claims.is_empty());
    assert!(
        !report.contradicted_claims.is_empty() || !report.needs_more_browsing_claims.is_empty(),
        "mixed qualifier block should not support the false default claim"
    );

    let maximum_report = extract_report(
        snapshot,
        vec![claim(
            "c2",
            "The maximum timeout for a Lambda function is 15 minutes.",
        )],
        "2026-04-09T00:00:00+09:00",
        SourceRisk::Low,
        Some("Lambda quotas".to_string()),
    );

    assert_supported_only(&maximum_report);
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
    serde_json::from_str(&fs::read_to_string(path).expect("fixture metadata should be readable"))
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
