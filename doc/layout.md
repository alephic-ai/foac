# Layout and conventions

`src/main.rs` parses the top-level CLI and dispatches. Provider commands are
colocated by provider: `src/linear.rs` contains Linear and `src/github.rs`
contains GitHub. Linear queries live in `graphql/linear/queries.graphql`, and
`graphql_client` generates their Rust types at compile time against the
vendored `graphql/linear/schema.graphql` (51k lines — grep it, don't read it
whole). It can be refreshed from
<https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql>.

Every command follows the same conventions: raw API JSON on stdout, errors on
stderr with exit code 1, nothing interactive. For Linear, `list` filter flags
accept a UUID or a human name (see `eq_filter`), while `create`/`update` flags
require UUIDs.

GitHub uses its versioned REST API. Successful object responses are raw JSON;
list responses wrap API items with `pageInfo` derived from the Link header.
