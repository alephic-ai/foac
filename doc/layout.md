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
  `graphql/linear/schema.graphql` (51k lines; grep it, don't read it whole).
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
- REST providers build on `src/rest.rs`: the shared `Api`/`send` (bearer or
  Basic auth, static provider headers, optional trailing slash), the
  `{items, pageInfo}` wrapper, payload helpers, and the auth-identity HTTP.
  Each provider keeps its own pagination parsing and list shapes, and prints
  lists through a provider-local `print_list`.
- GitHub uses its versioned REST API; list responses derive `pageInfo` from
  the Link header. Sentry follows the same REST pattern with cursor pagination
  from its Link header; its base URL comes from `SENTRY_URL`, then the host
  saved by `foac auth sentry login` (its prompt or `--host`; default
  `https://sentry.io`), and every request path needs a trailing slash.
- Neon uses REST API v2 with a bearer API key against
  `https://console.neon.tech`. Paginated lists (`projects`, `branches`,
  `operations`) read the next cursor from the response body's
  `pagination.cursor`; Neon returns that cursor even on the last page, so
  `hasNextPage` uses a full-page heuristic instead.
- Jira uses REST API v2 (plain-text bodies; v3 requires Atlassian Document
  Format) plus the Agile 1.0 API for sprints, authenticated with HTTP Basic
  (email + API token) against the tenant host. Issue search paginates with
  `nextPageToken`; other lists use `startAt`/`maxResults`. The three
  Atlassian credentials are stored under vendor-level `atlassian_*` keys,
  shared with Confluence.
- Confluence uses REST API v2 for spaces, pages, and footer comments, and the
  v1 root for CQL search (never ported to v2), with the same HTTP Basic
  Atlassian credentials as Jira. Bodies are written as wiki markup and read
  back in the storage representation; `page update` and `comment update`
  fetch the current version and re-send omitted fields, since the v2 PUT
  requires status, title, and body alongside the incremented version. v2
  lists paginate with a cursor extracted from `_links.next`; search uses
  `start`/`limit`.
- Slack uses its Web API's HTTP-200 `ok` envelope and cursor pagination.
  Ordinary commands resolve conversation and user names by paging through the
  relevant list method. Message search prefers `SLACK_USER_TOKEN`, then the
  stored user credential, because Slack does not permit bot tokens for
  `search.messages`.
- `provider.rs` keeps editable settings in comment-preserving `config.toml`
  and credentials in pretty-printed `credentials.json`, both under the XDG
  foac directory. It also reads the nearest `.foac.toml` from the working
  directory up to `/` and layers its `enabled_providers`/`disabled_providers`
  over the global toggles; `enable`/`disable` with `--local` edit that
  nearest file, creating one in the working directory when none exists. Reads and failures are isolated per store; writes use the
  shared atomic replacement path, with credentials mode `0600` before secret
  bytes are written on Unix. Legacy `config.json` is ignored.
