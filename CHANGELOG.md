# Changelog

All notable changes to this project are documented here.

## [0.2.1] - 2026-04-22

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v0.2.1

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.2.0...v0.2.1

### Fixed

- Corrected the MCP `tb_tab_open` output schema so target-based tab opens match the runtime response shape.
- Preserved telemetry details in compact `--agent-json` output for `telemetry-summary` and `telemetry-recent`.
- Made MCP search result opening recover the latest saved search tab after `tb_search_open_top` changes the active tab.
- Fixed static fixture navigation through catalog link aliases such as `/pricing`.

### Changed

- Bumped the Rust workspace, CLI runtime, MCP bridge, npm MCP package, Glama metadata, Docker install pin, and public server descriptors to `0.2.1`.

### Release Verification

- Local release checks passed before tagging: contract validation, eval smoke tests, full CLI crate tests, Rust fmt, and Clippy.

## [0.2.0] - 2026-04-22

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v0.2.0

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.1.13...v0.2.0

### Added

- Added the AI-facing `touch-browser capabilities` / `touch-browser status` command so agents can discover the runtime contract, supported surfaces, safety boundaries, output rules, and recommended first calls before browsing.
- Added global `--agent-json` output for compact agent-oriented JSON envelopes, including standardized `agentContract`, `nextActions`, `reuseSummary`, `sessionFile`, and compact search/session/evidence fields where available.
- Added structured `--json-errors` payloads with `retryable` and `suggestedAction` fields so agents can recover from usage, runtime, browser, verifier, and IO failures without parsing free-form stderr.
- Added `reuseAllowed` to evidence claim outcomes, derived from `evidence-supported + confidenceBand=high + reviewRecommended=false`, so agents get an explicit reuse signal instead of guessing.
- Added `primarySupportSnippet` and `supportSnippets[].supportRole` to evidence contracts so agents can separate the strongest supporting quote from surrounding context.
- Published aligned MCP package metadata and registry descriptors for `@nangman-infra/touch-browser-mcp@0.2.0`.

### Changed

- Bumped the Rust workspace, CLI runtime, MCP bridge, npm MCP package, Glama metadata, Docker install pin, and public server descriptors to `0.2.0`.
- Updated generated evidence golden fixtures for the new support snippet contract shape.
- Updated the CLI surface spec to document `capabilities`, `--agent-json`, `--json-errors`, standardized next actions, and evidence reuse fields.

### Fixed

- Formatted regenerated evidence golden fixtures so the repository lint gate passes cleanly.
- Relaxed a Playwright browser-backed search identity timeout in coverage mode to avoid CI-only timeout failures.
- Excluded Rust sources from Sonar coverage until Rust coverage is generated, preventing uncovered Rust source from being counted as zero-coverage JavaScript/TypeScript coverage.
- Reduced the Sonar cognitive complexity warning in the agent contract compaction path without changing output behavior.

### Release Verification

- Main quality gate passed on GitHub Actions for `5760f55`.
- Standalone release workflow passed for `v0.2.0`.
- GitHub Release assets were published for macOS arm64 and Linux x86_64, each with matching `.sha256` files.
- npm registry `latest` points to `@nangman-infra/touch-browser-mcp@0.2.0`.
- SonarQube Quality Gate is `OK` with `new_violations=0`.
