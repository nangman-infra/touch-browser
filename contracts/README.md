# Contracts

This directory is the canonical home for cross-process contracts.

Principles:

- the canonical source is the JSON Schema set under [contracts/schemas](schemas/README.md)
- Rust and TypeScript are consumer implementations of those contracts
- generated artifacts belong under `contracts/generated/`

Current workflow:

1. author or update JSON Schema
2. validate layout with `pnpm run contracts:check`
3. generate the schema manifest with `pnpm run contracts:manifest`
4. wire schema consumers in Rust and TypeScript

Expected generated artifacts:

- `contracts/generated/manifest.json`
- `contracts/generated/mcp-tool-catalog.json`
- `contracts/generated/mcp-tool-catalog.mjs`
- `contracts/generated/ts/*`
- `contracts/generated/rust/*`
