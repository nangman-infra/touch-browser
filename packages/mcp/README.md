# @nangman-infra/touch-browser-mcp

`@nangman-infra/touch-browser-mcp` is the npm-distributed local MCP entrypoint for `touch-browser`.

It is designed for public docs and research web workflows:

- search for official documentation
- open the top candidate tabs
- inspect `mainContentQuality` and `mainContentReason`
- extract evidence-supported or insufficient-evidence claims

This package does not expose `headed` or search-engine controls over MCP.

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

## Maintenance Commands

```bash
touch-browser-mcp install
touch-browser-mcp doctor
touch-browser-mcp bundle-path
```

## Registry Metadata

The MCP Registry metadata lives in `server.json` in this package directory and is intended for stdio package registration first. Remote MCP hosting is not part of this package yet.
