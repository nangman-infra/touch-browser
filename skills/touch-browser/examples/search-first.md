# Search-First Example

Use this when the user asks a question but does not provide the final page.

Example request:

> Find the official docs page that answers this question and verify the claim with citations.

Suggested flow:

1. run `search`
2. inspect ranked results and `nextActionHints`
3. open the best candidate with `search-open-result` or several with `search-open-top`
4. inspect `session-read`
5. run `session-extract`
6. synthesize only if several pages matter
