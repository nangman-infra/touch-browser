# Unresolved Claim Example

Use this when the extractor returns weak or incomplete evidence.

Example request:

> Verify this claim from the docs and tell me whether it is true.

If the result is `needs-more-browsing` or `insufficient-evidence`:

1. do not answer with certainty
2. browse to a more specific page if one is available
3. otherwise return unresolved with the current evidence state
