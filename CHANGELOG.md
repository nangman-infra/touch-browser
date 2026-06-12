# Changelog

All notable changes to this project are documented here.

## [1.0.0] - 2026-06-12

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v1.0.0

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.7.1...v1.0.0

### Added

- Added explicit `prompt-injection-attempt` policy signals for hostile pages, fake system/developer instructions, credential exfiltration prompts, and hidden instruction fixtures.
- Added prompt-injection guard coverage to the release readiness gate.

### Changed

- Promoted the internal readiness artifact to `product-ready` only when every tracked release gate passes.
- Tightened adversarial benchmark readiness so unsafe auto-answer counts must stay at zero and review capture must be complete.
- Bumped the Rust workspace, MCP npm package, MCP server descriptors, bridge metadata, workflow metadata, Glama Docker install pin, and standalone lifecycle expectations to `1.0.0`.
- Aligned the AWS Lambda adversarial sample with the current official overview page wording.

### Release Verification

- Local safety, adversarial, and release-readiness fixture generation passed.
- Local quality CI gate passed.
- Local public-web release gate passed.
- Local proof gate passed after regenerating proof artifacts.
- Local MCP package dry run passed.
- Local `v1.0.0` standalone bundle build and installed smoke passed.
- npm publishing is intentionally separate from this release because npm authentication was unavailable in the release environment.

## [0.7.0] - 2026-06-10

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v0.7.0

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.6.0...v0.7.0

### Added

- Added `touch-browser quickstart` as the beginner-safe local first-use guide with concrete success signals, safe defaults, and common pitfalls.
- Added `beginnerQuickStart` to `capabilities` / `status`, including the recommended local workflow from `doctor` through `quick`, `search`, `read-view`, and `extract`.
- Added top-level beginner-oriented `summary` fields for `open`, `quick`, and `extract`, plus top-level `claimOutcomes` for evidence commands.

### Changed

- Changed the recommended first call from `touch-browser capabilities --agent-json` to `touch-browser quickstart`.
- Bumped the Rust workspace, CLI runtime, MCP npm package, MCP server descriptors, workflow metadata, Glama Docker install pin, and standalone lifecycle expectations to `0.7.0`.
- Updated README and CLI surface docs so the visible first-use path matches the runtime output contract.

### Fixed

- Fixed the local-user footgun where passing a pre-created empty `--session-file` to browser-backed `open`, `quick`, or `extract` failed with a raw JSON EOF error.
- Improved empty persisted-session read errors so users get an actionable command for opening a page first.

### Release Verification

- Local `cargo fmt --all --check` passed.
- Local `cargo test -p touch-browser-cli --lib` passed.
- Local `pnpm run quality:lint`, `pnpm run quality:typecheck`, `cargo clippy --workspace --all-targets -- -D warnings`, `pnpm run quality:test:contracts`, `pnpm run quality:test:rust`, `pnpm run quality:test:adapter`, and `pnpm run quality:test:evals` passed.
- Local CLI smoke passed for `quickstart`, `status --agent-json`, `quick`, browser-backed `open` summary output, empty session-file recovery, and `quickstart --help`.

## [0.6.0] - 2026-05-05

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v0.6.0

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.5.0...v0.6.0

### Added

- Added `touch-browser quick` as the 60-second evidence check path for URL or fixture-backed claims.
- Added `tb_cancel`, MCP progress notifications, caller-provided `tb_session_create.sessionId`, and active-tab fallback behavior for `tb_open` when a session already has an opened tab.
- Added the `trusted-internal` source-risk tier and `custom:<key>` policy-signal namespace for enterprise policy integration.
- Added the Evidence-Grounded Web Research Benchmark seed, generator, and runtime benchmark gate.
- Added touch-connect and A2A integration specs for EvidenceReport handoff as `application/vnd.touch-browser.evidence-report+json`.

### Changed

- Made slim standalone bundles the default release profile, with semantic model downloads deferred until first embedding or NLI use.
- Clarified `claimOutcomes` versus session-level `evidenceSupportedClaims`, claim outcome reading order, verifier semantics, budget units, and memory crate boundaries in the docs.
- Published the current checked benchmark numbers in the README and benchmark docs so evidence quality is visible before adoption.
- Bumped the Rust workspace, CLI runtime, MCP npm package, MCP server descriptors, workflow client metadata, Glama Docker install pin, and standalone lifecycle expectations to `0.6.0`.

### Fixed

- Improved empty-session runtime errors so agents are told to open a tab first with `tb_open` or `tb_search_open_top`.
- Fixed benchmark accounting for citation unsupported cases, tool-comparison false positives, and adversarial contradiction routing.
- Fixed the evidence-grounded benchmark generator to use a typed error for numeric report-field validation.

### Release Verification

- Local `pnpm run quality:ci` passed before tagging.
- Local public-web QA gate passed before tagging.
- Local MCP package dry run passed before tagging.
- GitHub Actions `Quality Checks`, `Public Web QA`, and `SonarQube Scan` passed on `main` before tagging.

## [0.5.0] - 2026-04-26

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v0.5.0

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.4.0...v0.5.0

### Changed

- Raised the shared contract boundary to `1.1.0` so downstream consumers get an explicit migration signal for the split `pageRisk` / `actionRisk` policy shape.
- Promoted public-web QA from optional local-only coverage to GitHub workflow jobs on `main`, nightly schedule, and standalone release gating.
- Bumped the Rust workspace, CLI runtime, MCP npm package, MCP server descriptor, workflow client metadata, and standalone lifecycle expectations to `0.5.0`.

### Fixed

- Changed CLI telemetry to enterprise-safer defaults by making the default mode `redacted`, scrubbing persisted URLs down to origin-only, dropping claim payload persistence, and allowing `TOUCH_BROWSER_TELEMETRY_MODE=off|redacted|full`.

### Release Verification

- Local contract validation passed.
- Local CLI telemetry tests passed.
- Local public-web QA gate passed for MDN, Chrome Developers, IANA, and the multi-page follow flow.
- Local `cargo test -p touch-browser-cli` passed.
- Local `pnpm run quality:lint` passed.

## [0.4.0] - 2026-04-26

Release: https://github.com/nangman-infra/touch-browser/releases/tag/v0.4.0

Compare: https://github.com/nangman-infra/touch-browser/compare/v0.3.0...v0.4.0

### Added

- Added a real CLI E2E QA site regression gate for the fixture-backed multi-page follow flow.
- Added a public-web QA regression manifest with pinned expectations for MDN reference, Chrome Developers blog, IANA docs, and the multi-page follow workflow.
- Added an opt-in public-web CLI E2E regression path so real sites can be re-validated with `TOUCH_BROWSER_RUN_PUBLIC_WEB_QA=1`.
- Added a dedicated `v0.4.0` GitHub release-notes entry so release messaging can be kept in-repo instead of being reconstructed after the fact.

### Changed

- Split policy reporting into `pageRisk` and `actionRisk`, so read-only page review can stay separate from interaction-time supervision requirements.
- Updated checkpoint severity to reflect the more severe of page risk and action risk instead of only the top-level page decision.
- Upgraded the QA site regression surface from manifest pinning only to executable CLI gate coverage.
- Hardened Playwright navigation-action gate timing by raising the browser-backed follow timeout to a realistic CI-safe threshold.
- Bumped the Rust workspace, CLI runtime, MCP npm package, MCP server descriptor, workflow client metadata, and standalone lifecycle expectations to `0.4.0`.

### Fixed

- Fixed policy fixtures, contract examples, and regression tests so they match the split `pageRisk` / `actionRisk` model.
- Fixed false-high read-only risk on pages that merely contain login UI by reserving higher interaction risk for actual click/type/submit paths.
- Fixed human-facing `needs-more-browsing` explanations so unresolved claims now describe why the current page is too broad, indirect, or non-normative.
- Fixed search-session recovery so `runtime.search.openResult` can reopen results from the saved search tab even after the active tab changes.
- Reduced main-only extraction noise for `developer.mozilla.org` and `developer.chrome.com`, especially around language selectors, cookie/utility chrome, and footer clutter.
- Fixed standalone vs source CLI surface drift by re-verifying `capabilities`, `status`, and `--version` in release smoke coverage.

### Release Verification

- Local `pnpm run quality:ci` passed before tagging.
- GitHub Actions `Quality Checks` passed for `355f5fb` and `4880a88`.
- SonarQube quality gate is `OK` with `new_violations=0`, `new_coverage=81.2`, and `new_duplicated_lines_density=2.5596`.
- npm registry `latest` points to `@nangman-infra/touch-browser-mcp@0.4.0`.

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
