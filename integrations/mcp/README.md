# MCP Integration

Minimal bridge command:

```bash
node integrations/mcp/bridge/index.mjs
```

The checked-in desktop-style config example is:

- [claude-desktop.json](claude-desktop.json)

Supported MCP tools include:

- `tb_read_view`
- `tb_compact_view`
- `tb_extract`
- `tb_status`

The legacy launcher at `scripts/touch-browser-mcp-bridge.mjs` remains as a thin compatibility wrapper.
Set `TOUCH_BROWSER_SERVE_COMMAND` if you want the bridge to launch a built binary instead of the default `cargo run -q -p touch-browser-cli -- serve`.

For the protocol surface, see [MCP bridge spec](../../doc/MCP_BRIDGE_SPEC.md).
