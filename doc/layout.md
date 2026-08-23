# Layout and conventions

[ARCHITECTURE.md](../ARCHITECTURE.md) has the theory: what foac is, the module
map, the key decisions and the rules they impose, and where the system is
meant to grow. Read it first; this file keeps the mechanics it doesn't cover.

- `src/main.rs` parses the top-level CLI and dispatches. The modules live in a
  library target (`src/lib.rs`) so that `tests/cli.rs` (end-to-end runs of the
  compiled binary) and doc tests can link against the crate; unit tests stay
  inline in each module's `#[cfg(test)]` block.
- Linear queries live in `graphql/linear/queries.graphql`, and `graphql_client`
  generates their Rust types at compile time against the vendored
  `graphql/linear/schema.graphql` (51k lines — grep it, don't read it whole).
  It can be refreshed from
  <https://raw.githubusercontent.com/linear/linear/master/packages/sdk/src/schema.graphql>.
- Table-mode details of the shared printer (`src/output.rs`): the table is
  sized to the terminal width; enable and disable print the same keyed map as
  `provider list` with the changed provider's values bolded; single-provider
  auth status and login print a two-line summary (logout one line) while JSON
  stays a one-key map; the all-provider auth table flattens `account` to an
  identity string. Version, update, and skill output bypass the printer.
- For Linear, `list` filter flags accept a UUID or a human name (see
  `eq_filter`), while `create`/`update` flags require UUIDs.
- GitHub uses its versioned REST API; list responses derive `pageInfo` from
  the Link header. Sentry follows the same REST pattern with cursor pagination
  from its Link header; its base URL comes from `SENTRY_URL`, then the host
  saved by `foac auth sentry login` (its prompt or `--host`; default
  `https://sentry.io`), and every request path needs a trailing slash.
