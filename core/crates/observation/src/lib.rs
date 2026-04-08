mod budget;
mod hidden_rules;
mod semantic;
mod text;

use std::collections::{BTreeMap, HashMap};

use budget::apply_budget;
use hidden_rules::{is_hidden, HiddenRules};
use kuchiki::{parse_html, traits::TendrilSink, NodeRef};
use semantic::{
    candidate_priority, is_nested_duplicate, kind_slug, semantic_kind, semantic_role, semantic_tag,
    semantic_zone,
};
use serde_json::{json, Value};
use text::{
    candidate_slug, dom_path_hint, estimate_tokens, extract_semantic_text, extract_title,
    hostile_signal_hint, semantic_attributes,
};
use thiserror::Error;
use touch_browser_contracts::{
    SnapshotBlock, SnapshotBlockKind, SnapshotBlockRole, SnapshotBudget, SnapshotDocument,
    SnapshotEvidence, SnapshotSource, SourceType, CONTRACT_VERSION, STABLE_REF_VERSION,
};

pub fn crate_status() -> &'static str {
    "observation ready"
}

pub fn recommend_requested_tokens(html: &str, requested_tokens: usize) -> usize {
    if requested_tokens != 512 {
        return requested_tokens.max(1);
    }

    let html_len = html.len();
    let link_count = html.matches("<a").count();
    let heading_count = (1..=6)
        .map(|level| html.matches(&format!("<h{level}")).count())
        .sum::<usize>();
    let paragraph_count = html.matches("<p").count();
    let list_item_count = html.matches("<li").count();
    let table_count = html.matches("<table").count();
    let button_count = html.matches("<button").count();
    let input_count = html.matches("<input").count();

    let complexity_score = (html_len / 12_000)
        + (link_count / 20)
        + (heading_count / 10)
        + (paragraph_count / 30)
        + (list_item_count / 40)
        + (table_count * 3)
        + (button_count / 10)
        + (input_count / 10);

    if html_len >= 150_000 || link_count >= 120 || complexity_score >= 24 {
        4096
    } else if html_len >= 60_000 || link_count >= 45 || complexity_score >= 10 {
        2048
    } else if html_len >= 20_000 || link_count >= 20 || complexity_score >= 4 {
        1024
    } else {
        512
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationInput {
    pub source_url: String,
    pub source_type: SourceType,
    pub html: String,
    pub requested_tokens: usize,
}

impl ObservationInput {
    pub fn new(
        source_url: impl Into<String>,
        source_type: SourceType,
        html: impl Into<String>,
        requested_tokens: usize,
    ) -> Self {
        Self {
            source_url: source_url.into(),
            source_type,
            html: html.into(),
            requested_tokens,
        }
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ObservationCompiler;

impl ObservationCompiler {
    pub fn compile(&self, input: &ObservationInput) -> Result<SnapshotDocument, ObservationError> {
        if input.requested_tokens == 0 {
            return Err(ObservationError::ZeroBudget);
        }

        let document = parse_html().one(input.html.clone());
        let hidden_rules = HiddenRules::from_document(&document);
        let title = extract_title(&document);
        let mut candidates = collect_candidates(&document, input, &hidden_rules)?;

        if candidates.is_empty() {
            return Err(ObservationError::NoSemanticBlocks);
        }

        let estimated_tokens = candidates
            .iter()
            .map(|candidate| candidate.token_cost)
            .sum();
        let selected = apply_budget(input.requested_tokens, &mut candidates);
        let emitted_tokens = selected.iter().map(|candidate| candidate.token_cost).sum();
        let truncated = emitted_tokens < estimated_tokens;
        let blocks = selected
            .into_iter()
            .enumerate()
            .map(|(index, candidate)| candidate.into_snapshot_block(index + 1, input))
            .collect::<Vec<_>>();

        Ok(SnapshotDocument {
            version: CONTRACT_VERSION.to_string(),
            stable_ref_version: STABLE_REF_VERSION.to_string(),
            source: SnapshotSource {
                source_url: input.source_url.clone(),
                source_type: input.source_type.clone(),
                title,
            },
            budget: SnapshotBudget {
                requested_tokens: input.requested_tokens,
                estimated_tokens,
                emitted_tokens,
                truncated,
            },
            blocks,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ObservationError {
    #[error("observation input budget must be at least 1")]
    ZeroBudget,
    #[error("observation compiler found no semantic blocks")]
    NoSemanticBlocks,
    #[error("selector query failed: {0}")]
    InvalidSelection(String),
}

#[derive(Debug, Clone)]
pub(crate) struct CandidateBlock {
    order: usize,
    priority: usize,
    token_cost: usize,
    kind: SnapshotBlockKind,
    role: SnapshotBlockRole,
    stable_ref: String,
    text: String,
    attributes: BTreeMap<String, Value>,
    dom_path_hint: String,
}

impl CandidateBlock {
    fn into_snapshot_block(self, index: usize, input: &ObservationInput) -> SnapshotBlock {
        SnapshotBlock {
            version: CONTRACT_VERSION.to_string(),
            id: format!("b{index}"),
            kind: self.kind,
            stable_ref: self.stable_ref,
            role: self.role,
            text: self.text,
            attributes: self.attributes,
            evidence: SnapshotEvidence {
                source_url: input.source_url.clone(),
                source_type: input.source_type.clone(),
                dom_path_hint: Some(self.dom_path_hint),
                byte_range_start: None,
                byte_range_end: None,
            },
        }
    }
}

fn collect_candidates(
    document: &NodeRef,
    input: &ObservationInput,
    hidden_rules: &HiddenRules,
) -> Result<Vec<CandidateBlock>, ObservationError> {
    let mut ref_counts: HashMap<String, usize> = HashMap::new();
    let mut candidates = Vec::new();
    let mut order = 0usize;

    for node in document.descendants() {
        if node.as_element().is_none() {
            continue;
        }

        let Some(tag) = semantic_tag(&node) else {
            continue;
        };

        if is_hidden(&node, hidden_rules) || is_nested_duplicate(&node, &tag) {
            continue;
        }

        let text = extract_semantic_text(&node, &tag, hidden_rules)?;
        if text.is_empty() {
            continue;
        }

        let zone = semantic_zone(&node, &tag);
        let kind = semantic_kind(&tag);
        let role = semantic_role(&node, &tag, zone);
        let slug = candidate_slug(&node, &tag, &text);
        let base_ref = format!("r{zone}:{}:{slug}", kind_slug(&kind));
        let count = ref_counts.entry(base_ref.clone()).or_insert(0);
        *count += 1;

        let stable_ref = if *count == 1 {
            base_ref
        } else {
            format!("{base_ref}:{}", *count)
        };

        let mut attributes = semantic_attributes(&node, &tag, &text, hidden_rules)?;
        attributes.insert("zone".to_string(), json!(zone));
        attributes.insert("tagName".to_string(), json!(tag));
        if let Some(ancestor_signal) = hostile_signal_hint(input.source_url.as_str(), &text) {
            attributes.insert("hostileHint".to_string(), json!(ancestor_signal));
        }

        let priority = candidate_priority(&kind, &role);
        let token_cost = estimate_tokens(&text);
        let dom_path_hint = dom_path_hint(&node);

        candidates.push(CandidateBlock {
            order,
            priority,
            token_cost,
            kind,
            role,
            stable_ref,
            text,
            attributes,
            dom_path_hint,
        });
        order += 1;
    }

    candidates.sort_by_key(|candidate| candidate.order);
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde::Deserialize;

    use super::{recommend_requested_tokens, ObservationCompiler, ObservationInput};
    use touch_browser_contracts::{SnapshotDocument, SourceType};

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
        serde_json::from_str(
            &fs::read_to_string(path).expect("fixture metadata should be readable"),
        )
        .expect("fixture metadata should deserialize")
    }
}
