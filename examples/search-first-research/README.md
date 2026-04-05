# Search-First Research

This example shows the intended browser-first workflow:

1. search inside `touch-browser`
2. structure the result page into ranked candidates
3. open the best tab
4. switch to `read-view` or `extract`

```bash
# Search now uses a persistent embedded browser-backed profile by default and
# stores the search session under output/browser-search/google.search-session.json.
cargo run -q -p touch-browser-cli -- search "lambda timeout" --engine google

# For API or MCP workflows, keep the same search identity in a dedicated
# external profile while staying embedded/headless by default.
cargo run -q -p touch-browser-cli -- search "lambda timeout" --engine google \
  --profile-dir ~/Library/Application\\ Support/touch-browser/google-search-profile

# If the result reports `status: "challenge"`, rerun with --headed,
# clear the provider checkpoint in that same browser profile,
# then repeat the search.

# Open the first ranked result from that search session
cargo run -q -p touch-browser-cli -- search-open-result --rank 1

# Or open the top two recommended results into separate persisted sessions
cargo run -q -p touch-browser-cli -- search-open-top --limit 2

# Read the resulting page
cargo run -q -p touch-browser-cli -- session-read \
  --session-file output/browser-search/google.search-session.json \
  --main-only

# Extract a claim after the scope looks right
cargo run -q -p touch-browser-cli -- session-extract \
  --session-file output/browser-search/google.search-session.json \
  --claim "The maximum timeout for a Lambda function is 15 minutes."
```

For agent workflows, use the daemon and MCP bridge so the search tab can stay open while `tb_search_open_top` opens multiple candidate tabs in parallel.
