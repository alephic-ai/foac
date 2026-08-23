# Layout and conventions

`src/main.rs` parses the top-level CLI and dispatches. Provider commands are
colocated by provider: `src/linear.rs` contains Linear, `src/github.rs`
contains GitHub, and `src/sentry.rs` contains Sentry.
`src/update.rs` talks to GitHub Releases for `foac update` and
the once-a-day version check. Linear queries live in
`graphql/linear/queries.graphql`, and `graphql_client` generates their Rust
types at compile time against the vendored `graphql/linear/schema.graphql`
(51k lines — grep it, don't read it whole). It can be refreshed from
<https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql>.

Every command follows the same conventions: raw API JSON on stdout, errors on
stderr with exit code 1, nothing interactive. JSON success output (providers,
auth, provider enable/disable) goes through the shared printer in
`src/output.rs`: compact JSON by default, a table sized to the terminal width
when stdout is an interactive TTY (overridable with `--format json|table|auto`
or `FOAC_FORMAT`; `CI` forces JSON). Enable and disable print the same
keyed map as `provider list`; the table bolds the changed provider's values.
Single-provider auth status and login print a two-line summary in table mode
(logout prints one line); JSON stays a one-key map. The all-provider auth
table flattens `account` to an identity string.
Version, update, and skill output bypass it. For Linear, `list` filter flags
accept a UUID or a human name (see `eq_filter`), while `create`/`update` flags
require UUIDs.

GitHub uses its versioned REST API. Successful object responses are raw JSON;
list responses wrap API items with `pageInfo` derived from the Link header.
Sentry follows the same REST pattern with cursor pagination from its Link
header; its base URL comes from `SENTRY_URL`, then the host saved by
`foac auth sentry login` (its prompt or `--host`; default
`https://sentry.io`), and every request path needs a trailing slash.
