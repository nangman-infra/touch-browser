# MCP Integration

Recommended local-host path:

```bash
npx -y @nangman-infra/touch-browser-mcp
```

Installed standalone command:

```bash
touch-browser mcp
```

Repository integration asset:

```bash
node integrations/mcp/bridge/index.mjs
```

The npm package `@nangman-infra/touch-browser-mcp` is the primary local-host distribution path. It downloads the matching standalone runtime and starts `touch-browser mcp` for you.

The standalone installed bundle ships both `touch-browser mcp` and `touch-browser serve`. The checked-in Node launcher remains the repo-local integration asset.

The checked-in desktop-style config example is:

- [claude-desktop.json](claude-desktop.json)

Supported MCP tools include:

- `tb_search`
- `tb_search_open_top`
- `tb_read_view`
- `tb_extract`
- `tb_status`

MCP scope is public docs and research web. The bridge keeps browsing headless, does not expose `engine` or `headed`, and treats challenge/auth/MFA as supervised recovery handoff points rather than retriable browser settings.

The legacy launcher at `scripts/touch-browser-mcp-bridge.mjs` remains as a thin compatibility wrapper.
The bridge prefers `TOUCH_BROWSER_SERVE_COMMAND`, then `TOUCH_BROWSER_SERVE_BINARY`, then an installed or packaged `touch-browser` binary, then repo-local `target/{release,debug}` binaries.
If no binary is available, it fails fast and tells you to install a standalone bundle or build the repo once.
Set `TOUCH_BROWSER_SERVE_COMMAND` if you want to force a specific built binary or wrapper command.

For the protocol surface, see [MCP bridge spec](../../doc/MCP_BRIDGE_SPEC.md).
