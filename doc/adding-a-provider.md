# Adding a provider

The reference diff is the Neon addition (`git show 4576df0`): one provider
module plus a fixed set of registration and documentation touch points. Read
[ARCHITECTURE.md](../ARCHITECTURE.md) first; its key decisions (raw JSON
passthrough, uniform grammar, offline discovery, auth as a proxy) are the
rules every step below follows.

## 1. Pick the shape

- **REST** (the common case): untyped passthrough on `src/rest.rs`. Copy
  `src/sentry.rs` or `src/neon.rs`. Don't hand-write types for responses.
- **GraphQL with a published schema**: copy Linear's setup — vendor the
  schema under `assets/graphql/<provider>/schema.graphql`, write queries in
  `queries.graphql`, and generate types at compile time with
  `graphql_client` (see the `linear_query!` macro in `src/linear.rs`).
- **Neither fits** (e.g. Slack's HTTP-200 `ok`/error envelope): keep the
  protocol provider-local, as `src/slack.rs` does. That's a different
  protocol, not duplication.

## 2. Write `src/<provider>.rs`

One flat file (don't split into a module directory preemptively) containing:

- A `Cmd` clap tree: resources are the provider's own nouns, verbs are
  `list`/`get`/`create`/`update`/`delete`. `--help` is the API cache, so
  what a command accepts must never depend on runtime state.
- `run(cmd, format)`: builds a `rest::Api` (bearer or Basic auth, static
  headers, optional trailing slash) with the token from `auth.rs`, then
  dispatches. Arms return response JSON through `api.print` or the
  provider-local `print_list`; commands never print success output.
- A `path!` macro and `rest::push_query`/`rest::insert_opt` for building
  requests.
- `print_list`: wraps arrays with `rest::wrap_list` and the provider's own
  `pageInfo` parsing. Lists are `{"items": [...], "pageInfo": {...}}` with
  `--limit` plus the provider's cursor-or-page flag; everything else is the
  provider's raw JSON, forwarded as-is.
- `authenticated()`: cheap credential presence check (no network), used to
  hide the provider from help and the skill.
- `auth_identity(token)`: live validation via `rest::identity` against the
  provider's whoami endpoint.
- Inline `#[cfg(test)]` tests against `rest::testing::test_server`, never
  the real API: request shape (method, path, query, auth header, body) and
  pagination parsing.

## 3. Register it everywhere

- `src/lib.rs`: `pub mod <provider>;` (alphabetical).
- `src/main.rs`: the `use foac::{...}` import, a `Command` variant with a
  `/// Interact with <Provider>` doc comment, its dispatch arm through
  `provider::ensure_enabled`, and the
  `help_only_lists_authenticated_providers` test.
- `src/auth.rs`: an `AuthCmd` variant and dispatch arm; a `Provider` enum
  variant (alphabetical: the enum order is the `provider list`, `auth
  status`, and skill order) with its `info()` arm (name, display name, env
  var, credential, and the module's `authenticated` fn — every other
  provider list derives from that one match); a `<provider>_token(instance)`
  resolver (env var for the default instance, then the credentials file); a
  `validate` arm mapping `auth_identity` to an account object (safe fields
  only — never token material); an identity formatter for the auth table;
  an arm in `provider_status`; and a `print_login_help` arm telling the
  user where to create a token.
- `src/provider.rs`: a `Credential` variant with its `vendor`/`field` keys,
  and the provider-list test fixture.
  Providers sharing vendor-level credentials reuse one vendor, like the
  Atlassian pair, whose shared code lives in `src/atlassian.rs`.
- `tests/cli.rs`: the provider names in the `provider list` test.
  End-to-end tests stay parse-clean: no real auth, no network.

## 4. Document it in the same change

- `assets/SKILL.md`: wrap each addition in
  `<!-- foac-provider:<name> -->` … `<!-- /foac-provider:<name> -->` marker
  blocks — a frontmatter `name: foac-<provider>` / `description:` pair, an
  H1, the provider bullet under the grammar, an auth-precedence bullet,
  any provider-specific conventions (scoping flags, pagination,
  identifier shapes), and an example block. CI validates every rendered
  skill.
- `.github/workflows/skill.yml`: add the provider to the `for p in ...`
  validation loop.
- `doc/<provider>.md`: what the provider covers, auth, examples,
  pagination, what is deliberately not covered, and an entity-relationship
  `erDiagram` like the other provider docs.
- `doc/auth.md`: the `foac auth <provider> ...` command list.
- `doc/layout.md`: one bullet on the provider's API version, auth style,
  and pagination quirks.
- `ARCHITECTURE.md`: the intro provider list, the module diagram, and the
  codemap.
- `README.md`: the intro provider list and the provider table row only.
  Leave the two mermaid diagrams and their integration count alone: they
  illustrate the idea with a fixed set of providers and must not grow with
  every addition.

## 5. Ship it

Commit with a `feat:` prefix — version bumps derive from
conventional-commit prefixes, and a push to main publishes the release
(`doc/releasing.md`).

New resources or verbs on an existing provider are the smaller recipe:
[adding-a-command.md](adding-a-command.md).
