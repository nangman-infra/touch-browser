use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::*;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FixtureMetadata {
    title: String,
    source_uri: String,
    html_path: String,
    risk: String,
}

pub(crate) fn load_fixture_catalog() -> Result<FixtureCatalog, CliError> {
    let mut catalog = FixtureCatalog::default();

    for metadata_path in fixture_metadata_paths()? {
        let metadata: FixtureMetadata = serde_json::from_str(&fs::read_to_string(&metadata_path)?)?;
        let html = fs::read_to_string(repo_root().join(metadata.html_path))?;
        let risk = parse_fixture_risk(&metadata.risk)?;

        catalog.register(
            CatalogDocument::new(
                metadata.source_uri.clone(),
                html,
                SourceType::Fixture,
                risk,
                Some(metadata.title),
            )
            .with_aliases(default_aliases(&metadata.source_uri)),
        );
    }

    Ok(catalog)
}

fn fixture_metadata_paths() -> Result<Vec<PathBuf>, CliError> {
    let research_root = repo_root().join("fixtures/research");
    let mut paths = Vec::new();

    for category in fs::read_dir(research_root)? {
        let category = category?;
        if !category.file_type()?.is_dir() {
            continue;
        }

        for fixture in fs::read_dir(category.path())? {
            let fixture = fixture?;
            if !fixture.file_type()?.is_dir() {
                continue;
            }

            let metadata_path = fixture.path().join("fixture.json");
            if metadata_path.is_file() {
                paths.push(metadata_path);
            }
        }
    }

    paths.sort();
    Ok(paths)
}

fn default_aliases(source_uri: &str) -> Vec<String> {
    match source_uri {
        "fixture://research/static-docs/getting-started" => {
            vec!["/docs".to_string(), "/getting-started".to_string()]
        }
        "fixture://research/citation-heavy/pricing" => vec!["/pricing".to_string()],
        "fixture://research/navigation/api-reference" => {
            vec!["/api".to_string(), "/api-reference".to_string()]
        }
        _ => Vec::new(),
    }
}

fn parse_fixture_risk(value: &str) -> Result<SourceRisk, CliError> {
    match value {
        "low" => Ok(SourceRisk::Low),
        "medium" => Ok(SourceRisk::Medium),
        "hostile" => Ok(SourceRisk::Hostile),
        _ => Err(CliError::Usage(format!(
            "fixture metadata contains unknown risk `{value}`."
        ))),
    }
}
