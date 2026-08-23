# Layout and conventions

The layout is two files: `src/main.rs` parses the top-level CLI and
dispatches, `src/linear.rs` holds every Linear command. Queries live in
`graphql/linear/queries.graphql`, and `graphql_client` generates their Rust
types at compile time against the vendored `graphql/linear/schema.graphql`
(51k lines — grep it, don't read it whole). It can be refreshed from
<https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql>.

Every command follows the same conventions: raw API JSON on stdout, errors on
stderr with exit code 1, nothing interactive. `list` filter flags accept a
UUID or a human name (see `eq_filter`); `create`/`update` flags require UUIDs.
