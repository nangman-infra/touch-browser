# Search-First Research

This example shows the intended browser-first workflow:

1. search inside `touch-browser`
2. structure the result page into ranked candidates
3. open the best tab
4. switch to `read-view` or `extract`

```bash
# Save a search session
cargo run -q -p touch-browser-cli -- search "lambda timeout" \
  --engine google \
  --session-file /tmp/touch-browser-search.json

# If the result reports `status: "challenge"`, rerun with --headed,
# clear the provider checkpoint manually, then repeat the search.

# Open the first ranked result from that saved search
cargo run -q -p touch-browser-cli -- search-open-result \
  --session-file /tmp/touch-browser-search.json \
  --rank 1

# Read the resulting page
cargo run -q -p touch-browser-cli -- session-read \
  --session-file /tmp/touch-browser-search.json \
  --main-only

# Extract a claim after the scope looks right
cargo run -q -p touch-browser-cli -- session-extract \
  --session-file /tmp/touch-browser-search.json \
  --claim "The maximum timeout for a Lambda function is 15 minutes."
```

For agent workflows, use the daemon and MCP bridge so the search tab can stay open while `tb_search_open_top` opens multiple candidate tabs in parallel.
