use std::{fs, path::PathBuf};

use serde::Deserialize;
use touch_browser_contracts::{SnapshotDocument, SourceType};

use super::{recommend_requested_tokens, ObservationCompiler, ObservationInput};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMetadata {
    source_uri: String,
    html_path: String,
    expected_snapshot_path: String,
}

#[test]
fn produces_expected_golden_snapshots_for_seed_fixtures() {
    let compiler = ObservationCompiler;

    for fixture in seed_fixture_paths() {
        let metadata = read_fixture_metadata(&fixture);
        let html_path = repo_root().join(metadata.html_path);
        let expected_path = repo_root().join(metadata.expected_snapshot_path);

        let actual = compiler
            .compile(&ObservationInput::new(
                metadata.source_uri,
                SourceType::Fixture,
                fs::read_to_string(html_path).expect("fixture html should be readable"),
                512,
            ))
            .expect("fixture snapshot should compile");

        let expected: SnapshotDocument = serde_json::from_str(
            &fs::read_to_string(expected_path).expect("golden snapshot should be readable"),
        )
        .expect("golden snapshot json should deserialize");

        assert_eq!(actual, expected);
    }
}

#[test]
fn generates_deterministic_refs_for_same_input() {
    let compiler = ObservationCompiler;
    let fixture = repo_root().join("fixtures/research/static-docs/getting-started/index.html");
    let html = fs::read_to_string(fixture).expect("fixture html should be readable");
    let input = ObservationInput::new(
        "fixture://research/static-docs/getting-started",
        SourceType::Fixture,
        html,
        512,
    );

    let first = compiler.compile(&input).expect("first compile should work");
    let second = compiler
        .compile(&input)
        .expect("second compile should work");

    let first_refs = first
        .blocks
        .iter()
        .map(|block| block.stable_ref.as_str())
        .collect::<Vec<_>>();
    let second_refs = second
        .blocks
        .iter()
        .map(|block| block.stable_ref.as_str())
        .collect::<Vec<_>>();

    assert_eq!(first_refs, second_refs);
}

#[test]
fn truncates_low_priority_blocks_when_budget_is_small() {
    let compiler = ObservationCompiler;
    let fixture = repo_root().join("fixtures/research/static-docs/getting-started/index.html");
    let html = fs::read_to_string(fixture).expect("fixture html should be readable");

    let snapshot = compiler
        .compile(&ObservationInput::new(
            "fixture://research/static-docs/getting-started",
            SourceType::Fixture,
            html,
            12,
        ))
        .expect("compile should work");

    assert!(snapshot.budget.truncated);
    assert!(snapshot.budget.emitted_tokens < snapshot.budget.estimated_tokens);
    assert!(snapshot
        .blocks
        .iter()
        .any(|block| block.text.contains("Getting Started")));
    assert!(snapshot.blocks.iter().all(|block| block.text != "Pricing"));
}

#[test]
fn recommends_higher_budget_for_complex_pages() {
    let simple_html = "<html><body><main><h1>Simple</h1><p>Hello</p></main></body></html>";
    assert_eq!(recommend_requested_tokens(simple_html, 512), 512);

    let large_html = format!(
        "<html><body>{}</body></html>",
        (0..160)
            .map(|index| format!(
                "<a href=\"/docs/{index}\">Doc {index}</a><p>Paragraph {index}</p>"
            ))
            .collect::<String>()
    );
    assert!(
        recommend_requested_tokens(&large_html, 512) >= 2048,
        "complex pages should auto-escalate beyond the default budget"
    );
    assert_eq!(recommend_requested_tokens(&large_html, 4096), 4096);
}

#[test]
fn captures_json_ld_and_hydration_scripts_as_semantic_metadata() {
    let compiler = ObservationCompiler;
    let html = r#"
            <html>
              <head>
                <title>Modern Docs</title>
                <script type="application/ld+json">
                  {"@context":"https://schema.org","@type":"TechArticle","headline":"Modern Docs","datePublished":"2026-04-05"}
                </script>
              </head>
              <body>
                <main>
                  <h1>Modern Docs</h1>
                  <p>Primary content.</p>
                </main>
                <script id="__NEXT_DATA__" type="application/json">
                  {"page":"/docs","buildId":"build-123","props":{"pageProps":{"title":"Modern Docs","slug":"modern-docs"}}}
                </script>
              </body>
            </html>
        "#;

    let snapshot = compiler
        .compile(&ObservationInput::new(
            "https://docs.example.com/modern",
            SourceType::Http,
            html,
            512,
        ))
        .expect("compile should work");

    let metadata_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| block.kind == touch_browser_contracts::SnapshotBlockKind::Metadata)
        .collect::<Vec<_>>();

    assert!(metadata_blocks
        .iter()
        .any(|block| block.text.contains("json-ld")));
    assert!(metadata_blocks
        .iter()
        .any(|block| block.text.contains("hydration")));
    assert!(metadata_blocks
        .iter()
        .any(|block| block.text.contains("Modern Docs")));
}

#[test]
fn strips_hidden_noscript_markup_from_visible_text_blocks() {
    let compiler = ObservationCompiler;
    let html = r#"
            <html>
              <body>
                <main>
                  <h1>Downloads</h1>
                  <p>
                    Get Node.js v24.14.1 (LTS)
                    <noscript>
                      <style>.select-hidden { display: none !important; }</style>
                      <div class="index-module__select">macOS Windows Linux</div>
                    </noscript>
                  </p>
                </main>
              </body>
            </html>
        "#;

    let snapshot = compiler
        .compile(&ObservationInput::new(
            "https://example.com/download",
            SourceType::Playwright,
            html,
            512,
        ))
        .expect("compile should work");

    let text_block = snapshot
        .blocks
        .iter()
        .find(|block| block.text.contains("Get Node.js"))
        .expect("visible paragraph block should exist");
    assert!(text_block.text.contains("Get Node.js v24.14.1 (LTS)"));
    assert!(!text_block.text.contains("<style>"));
    assert!(!text_block.text.contains("index-module__select"));
    assert!(!text_block.text.contains("macOS Windows Linux"));
}

#[test]
fn captures_selectors_as_semantic_option_blocks() {
    let compiler = ObservationCompiler;
    let html = r#"
            <html>
              <body>
                <main>
                  <h1>Downloads</h1>
                  <button
                    type="button"
                    role="combobox"
                    aria-label="Platform"
                    aria-expanded="true"
                    aria-controls="platform-list"
                  >
                    Linux
                  </button>
                  <ul id="platform-list" role="listbox">
                    <li role="option">macOS</li>
                    <li role="option">Windows</li>
                    <li role="option">Linux</li>
                  </ul>
                </main>
              </body>
            </html>
        "#;

    let snapshot = compiler
        .compile(&ObservationInput::new(
            "https://example.com/download",
            SourceType::Playwright,
            html,
            512,
        ))
        .expect("compile should work");

    let selector_blocks = snapshot
        .blocks
        .iter()
        .filter(|block| {
            matches!(
                block.attributes.get("selectionSemantic"),
                Some(serde_json::Value::String(value)) if value == "available-options"
            )
        })
        .collect::<Vec<_>>();

    assert!(selector_blocks
        .iter()
        .any(|block| block.text.contains("current Linux")));
    assert!(selector_blocks
        .iter()
        .any(|block| block.text.contains("macOS")));
}

#[test]
fn keeps_main_intro_text_when_header_links_are_dense() {
    let compiler = ObservationCompiler;
    let header_links = (0..80)
        .map(|index| format!("<a href=\"https://example.com/lang/{index}\">Lang {index}</a>"))
        .collect::<String>();
    let html = format!(
        r#"
            <html>
              <body>
                <header class="language-selector">{header_links}</header>
                <main>
                  <h1>Moon landing</h1>
                  <p>A Moon landing or lunar landing is the arrival of a spacecraft on the Moon.</p>
                  <p>In 1969, Apollo 11 was the first crewed mission to land on the Moon.</p>
                  <div class="mw-heading"><h2>Human Moon landings</h2></div>
                  <table>
                    <tr><th>Mission</th><th>Date</th></tr>
                    <tr><td>Apollo 11</td><td>20 July 1969</td></tr>
                    <tr><td>Apollo 12</td><td>19 November 1969</td></tr>
                  </table>
                </main>
              </body>
            </html>
        "#
    );

    let snapshot = compiler
        .compile(&ObservationInput::new(
            "https://en.wikipedia.org/wiki/Moon_landing",
            SourceType::Http,
            html,
            128,
        ))
        .expect("compile should work");

    assert!(snapshot.budget.truncated);
    assert!(snapshot.blocks.iter().any(|block| {
        block.kind == touch_browser_contracts::SnapshotBlockKind::Text
            && block
                .text
                .contains("A Moon landing or lunar landing is the arrival")
    }));
    assert!(snapshot.blocks.iter().any(|block| {
        block.kind == touch_browser_contracts::SnapshotBlockKind::Text
            && block
                .text
                .contains("Apollo 11 was the first crewed mission")
    }));
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .canonicalize()
        .expect("repo root should exist")
}

fn seed_fixture_paths() -> Vec<PathBuf> {
    vec![
        repo_root().join("fixtures/research/static-docs/getting-started/fixture.json"),
        repo_root().join("fixtures/research/navigation/api-reference/fixture.json"),
        repo_root().join("fixtures/research/citation-heavy/pricing/fixture.json"),
        repo_root().join("fixtures/research/hostile/hidden-instruction/fixture.json"),
        repo_root().join("fixtures/research/hostile/fake-system-message/fixture.json"),
    ]
}

fn read_fixture_metadata(path: &PathBuf) -> FixtureMetadata {
    serde_json::from_str(&fs::read_to_string(path).expect("fixture metadata should be readable"))
        .expect("fixture metadata should deserialize")
}
