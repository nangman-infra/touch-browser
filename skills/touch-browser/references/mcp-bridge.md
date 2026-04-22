# MCP Bridge Notes

Use this reference when the workflow is running through MCP instead of direct CLI commands.

Key boundary:

- MCP is narrower than the full CLI
- headed browsing is not exposed over MCP
- challenge, auth, and MFA states are handoff conditions, not retry signals

Recommended MCP loop:

1. `tb_search`
2. `tb_search_open_top`
3. `tb_read_view`
4. `tb_extract`

Read the full contract in:

- [../../../doc/MCP_BRIDGE_SPEC.md](../../../doc/MCP_BRIDGE_SPEC.md)
