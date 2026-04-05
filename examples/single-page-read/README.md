# Single Page Read

Use this example when the task starts with a single public document.

```bash
cargo run -q -p touch-browser-cli -- read-view https://www.iana.org/help/example-domains --main-only

cargo run -q -p touch-browser-cli -- compact-view https://www.iana.org/help/example-domains --allow-domain www.iana.org

cargo run -q -p touch-browser-cli -- extract https://www.iana.org/help/example-domains \
  --allow-domain www.iana.org \
  --claim "As described in RFC 2606 and RFC 6761, a number of domains such as example.com and example.org are maintained for documentation purposes."
```

What this shows:

- `read-view` is the readable markdown surface
- `compact-view` is the low-token routing surface
- `extract` is the evidence surface with citations and final claim outcomes
