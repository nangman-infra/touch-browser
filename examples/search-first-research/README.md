# Search-First Research

This example shows the intended browser-first workflow:

1. search inside `touch-browser`
2. structure the result page into ranked candidates
3. open the best tab
4. switch to `read-view` or `extract`

```bash
# Search now chooses the engine automatically, uses a persistent
# embedded browser-backed profile by default, and can store the
# search session in an explicit file.
cargo run -q -p touch-browser-cli -- search "lambda timeout" \
  --session-file /tmp/lambda.search-session.json

# For repeated CLI verification, keep the same search identity in a dedicated
# external profile while staying embedded/headless by default.
cargo run -q -p touch-browser-cli -- search "lambda timeout" \
  --session-file /tmp/lambda.search-session.json \
  --profile-dir ~/Library/Application\\ Support/touch-browser/google-search-profile

# If the result reports `status: "challenge"`, use the same profile in a
# supervised human recovery path, clear the provider checkpoint there,
# then repeat the search. MCP clients should stop and request recovery
# instead of retrying with headed browser settings.

# Open the first ranked result from that search session
cargo run -q -p touch-browser-cli -- search-open-result \
  --session-file /tmp/lambda.search-session.json \
  --rank 1

# Or open the top two recommended results into separate persisted sessions
cargo run -q -p touch-browser-cli -- search-open-top \
  --session-file /tmp/lambda.search-session.json \
  --limit 2

# Read the resulting page
cargo run -q -p touch-browser-cli -- session-read \
  --session-file /tmp/lambda.search-session.json \
  --main-only

# Extract a claim after the scope looks right
cargo run -q -p touch-browser-cli -- session-extract \
  --session-file /tmp/lambda.search-session.json \
  --claim "The maximum timeout for a Lambda function is 15 minutes."
```

For agent workflows, use the daemon and MCP bridge so the search tab can stay open while `tb_search_open_top` opens multiple candidate tabs in parallel. Over MCP, engine selection stays automatic and headed browsing is not exposed.
