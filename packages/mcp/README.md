# @nangman-infra/touch-browser-mcp

`@nangman-infra/touch-browser-mcp` is the npm-distributed local MCP entrypoint for `touch-browser`.

It is designed for public docs and research web workflows:

- search for official documentation
- open the top candidate tabs
- inspect `mainContentQuality` and `mainContentReason`
- extract evidence-supported or insufficient-evidence claims

This package does not expose `headed` or search-engine controls over MCP.

## What To Check First

For a direct URL, the MCP equivalent of the CLI `touch-browser quick` path is:

1. `tb_session_create`
2. `tb_open`
3. `tb_extract`

Read claim outcomes in this order:

1. `verdict`
2. `reviewRecommended`
3. `primarySupportSnippet`
4. `verdictExplanation`
5. `citation`

Use `matchSignals` for debugging or quality comparison, not as the first field a new user needs to read.

## First MCP Loop

Use this tool order for a direct URL:

1. `tb_session_create`
2. `tb_open` with `sessionId` and `target`
3. `tb_read_view` with `sessionId`
4. `tb_extract` with `sessionId` and `claims`
5. `tb_session_synthesize` with `sessionId`
6. `tb_session_close` with `sessionId`

Argument model:

- `tb_session_create` accepts an optional caller-provided `sessionId` for external correlation.
- `tb_open` requires `target` for stateless use; with `sessionId`, it can omit `target` to reopen the active tab URL.
- `tb_read_view`, `tb_extract`, and `tb_policy` can omit `target` when `sessionId` points at an opened active tab.
- `tb_extract` always requires `claims`.
- `tb_session_synthesize` requires `sessionId` and at least one opened tab.
- `tb_cancel` is a best-effort daemon reset; MCP hosts should use `notifications/cancelled` for an in-flight call.

If a session has no opened tab yet, call `tb_open` or `tb_search_open_top` first.

Long-running calls emit MCP `notifications/progress` when the host provides `_meta.progressToken`.

## Host Config

Run directly through `npx`:

```json
{
  "mcpServers": {
    "touch-browser": {
      "command": "npx",
      "args": ["-y", "@nangman-infra/touch-browser-mcp"]
    }
  }
}
```

Or install globally:

```bash
npm install -g @nangman-infra/touch-browser-mcp
touch-browser-mcp
```

## First Run

On first launch, the package downloads the matching standalone `touch-browser` bundle for the current package version from GitHub Releases, verifies the published `.sha256`, extracts it under:

```text
~/.touch-browser/npm-mcp/versions/
```

and then starts `touch-browser mcp`.

The published standalone bundle uses the slim release profile by default. That keeps semantic model caches lazy, but the first run can still take time because the runtime bundle and browser dependencies may need to be downloaded.

## Maintenance Commands

```bash
touch-browser-mcp install
touch-browser-mcp doctor
touch-browser-mcp bundle-path
```

## Registry Metadata

The MCP Registry metadata lives in `server.json` in this package directory and is intended for stdio package registration first. Remote MCP hosting is not part of this package yet.
