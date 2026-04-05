# Schemas

The JSON Schema files in this directory are the source of truth for cross-process contracts.

Initial priority order:

1. `snapshot-block.schema.json`
2. `snapshot-document.schema.json`
3. `evidence-block.schema.json`
4. `unsupported-claim.schema.json`
5. `evidence-report.schema.json`
6. `action-command.schema.json`
7. `session-state.schema.json`
8. `replay-transcript.schema.json`
9. JSON-RPC envelopes

Notes:

- follow the schema-first approach before expanding Rust or TypeScript consumers
- lock boundary payloads in schema form first, then implement the producers and consumers
