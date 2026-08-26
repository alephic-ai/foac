# foac

foac, the Father Of All CLIs, wraps external SaaS APIs (Linear, GitHub, Jira,
Confluence, Neon, Sentry, Slack, Vercel, more to come) behind one command grammar:
`foac <provider> <resource> <verb>`. The primary consumer is an LLM agent
working in a shell. Humans at a TTY get a rendering layer on top of the same
output. foac makes any provider's API discoverable, uniform, and already
authenticated. It does not abstract those APIs or try to improve them.

## How the system maps to the world

Each first-level subcommand is a provider: an external product's API.
Resources are the provider's own nouns (`issue`, `pull`, `release`); verbs are
`list`/`get`/`create`/`update`/`delete`. The compiled-in clap command tree
(plus, for Linear, the vendored GraphQL schema) is a local cache of each
provider's API structure. An agent discovers the API through `--help` with no
network round-trip, which keeps discovery deterministic, fast, and
token-cheap. Response bodies are the provider's raw JSON. foac does not
reshape what the API returned, so upstream API docs remain valid documentation
for foac's output.

Three cross-provider concerns are foac's own: auth (log in once, every agent
harness and script reuses the credential), discovery (help text and a
self-installing agent skill that only show authenticated, enabled providers),
and output (JSON for machines, tables for humans, decided per invocation).

## Architecture

```text
                 main.rs  (parse, dispatch, provider hiding, skill render)
                    │
   ┌────────────┬────────────┬───────────────┬───────────────┬────────────┬────────────┬─────────────┐
   │ linear.rs  │ github.rs  │ jira.rs       │ neon.rs       │ slack.rs   │ vercel.rs  │ auth.rs     │
   │ (GraphQL,  │ (REST,     │ confluence.rs │ sentry.rs     │ (REST,     │ (REST,     │ provider.rs │
   │  codegen)  │  untyped)  │ (REST,untyped)│ (REST,untyped)│  untyped)  │  untyped)  │ update.rs   │
   └─────┬──────┴─────┬──────┴───────┬───────┴───────┬───────┴─────┬──────┴─────┬──────┴─────────────┘
         └────────────┴──────────────┴───────────────┴────────────┴──────────┬───┘
                                                                            ▼
                        output.rs  (single shared printer: JSON | table)
```

Every provider command builds a request, gets back JSON, and returns it to its
`run` function, which prints exactly once through `output.rs`. Auth resolution
(`auth.rs`) and the independent stores in `provider.rs` sit under all
providers: editable settings in `~/.config/foac/config.toml`, and
machine-managed credentials in `~/.config/foac/credentials.json`.

## Codemap

```text
src/
├── main.rs      # CLI root: dispatch, hiding inactive providers, skill render/install
├── linear.rs    # Linear provider: GraphQL via graphql_client codegen
├── github.rs    # GitHub provider: REST passthrough on rest.rs
├── jira.rs      # Jira provider: REST passthrough on rest.rs, Basic auth
├── confluence.rs # Confluence provider: REST passthrough on rest.rs, shares Jira's Atlassian auth
├── neon.rs      # Neon provider: REST passthrough on rest.rs
├── sentry.rs    # Sentry provider: REST passthrough on rest.rs
├── slack.rs     # Slack provider: REST with Slack's HTTP-200 ok/error envelope
├── vercel.rs    # Vercel provider: versioned REST passthrough on rest.rs
├── rest.rs      # Shared REST core: Api/send, list wrapping, payload helpers, auth-identity HTTP
├── auth.rs      # Credential resolution (env > credentials file > gh CLI), validation, auth commands
├── provider.rs  # Comment-preserving TOML settings + private JSON credentials
├── output.rs    # The one success printer: compact JSON, or shape-heuristic tables on a TTY
├── update.rs    # Self-update from GitHub Releases + once-a-day version check
└── lib.rs       # Library target so tests/ and doc tests can link; main.rs is the only consumer
assets/
├── graphql/linear/ # Vendored schema (51k lines; grep it) + queries.graphql (compile-time checked)
└── SKILL.md     # Agent skill, compiled into the binary; must track every CLI surface change
```

`rest.rs` owns the REST boilerplate: the `Api` struct and `send` (bearer or
Basic auth, static provider headers, optional trailing slash), the
`{items, pageInfo}` list wrapper, payload helpers, and the auth-identity
HTTP. Providers keep only what is genuinely theirs (pagination parsing, list
shapes, ID resolution). Slack's `send` stays provider-local on purpose: its
HTTP-200 `ok`/error envelope and method-string URLs are a different protocol,
not duplication.

## Key decisions

- Raw JSON passthrough. Each provider is assumed to use the data shape most
  adequate for its domain, so foac forwards responses as-is. Agents can trust
  the fields against upstream docs. The one standing normalization is wrapping
  REST lists as `{"items": [...], "pageInfo": {...}}`. Further reshaping is
  allowed only when it clearly earns its keep (for example, GitHub issue lists
  filter out pull requests).
- The command tree is a compiled-in API cache. `--help` at every level shows
  what exists, generated from clap definitions checked at compile time (and,
  for Linear, queries validated against the vendored schema). Discovery must
  stay offline and deterministic. Never make what a command accepts depend on
  runtime state.
- Uniform grammar across providers: provider/resource/verb, `--limit` plus
  cursor-or-page flags, `{items, pageInfo}` lists, JSON errors on stderr with
  exit 1. Shaping one provider differently from the others breaks
  deterministic discovery. That uniformity is what keeps foac fast to use
  with many providers enabled.
- Codegen only when the provider publishes a schema. Linear (GraphQL) gets
  compile-time-checked queries via `graphql_client`. REST providers stay
  untyped passthrough. The less to maintain in foac, the better. Don't
  hand-write types for REST responses.
- Every provider holds named instances: independent logins to different
  tenants of the same product (two Slack workspaces, two Atlassian sites).
  An unnamed login is the instance named `default`, which unqualified
  commands use, so single-tenant setups never see the concept. A command's
  instance resolves as `--instance` flag > nearest `[defaults]` table
  (local `.foac.toml`, then global `config.toml`)
  > `default`, computed once in `main.rs` beside `ensure_enabled` and
  threaded into each provider's `run`. Auth commands take the flag only, so
  a login is never silently redirected. `provider@instance` is the qualified
  grammar wherever an instance is data: toggle arrays, status keys, errors.
- Auth is a proxy, resolved per provider as env var > credentials file (> `gh`
  CLI for GitHub) for the default instance; a named instance reads exactly
  its stored credentials — env tokens and the `gh` fallback never apply, so
  an ambient token from one tenant cannot leak into another. Log in once,
  reuse everywhere. Credentials live in the atomically replaced
  `credentials.json`, nested provider → instance → fields, mode 0600 before
  secret bytes are written on Unix, not the OS keychain (rebuilt binaries
  lose macOS Keychain ACL trust, #35). The `atlassian` entry is vendor-level,
  shared by Jira and Confluence; a Sentry instance stores its base URL with
  its token. Tokens are never printed. Login validates before storing.
  Slack stores bot and user credentials independently. Ordinary commands resolve
  bot env > stored bot > user env > stored user; search resolves user env >
  stored user because Slack does not allow bot tokens to call `search.messages`.
  Slack login asks for both and validates the pair before one atomic credential
  write. This makes bot-only, user-only, both-token, and unauthenticated
  installations explicit capability modes.
- Settings and credentials fail closed independently. Provider toggles and
  the `[defaults]` instance table live in comment-preserving `config.toml`;
  credentials for every provider live in pretty-printed `credentials.json`.
  Unknown TOML keys are discarded on write. Legacy `config.json`, and the
  flat pre-instance `credentials.json` keys, are deliberately ignored
  fresh-start formats: never read, migrated, deleted, or used as fallback.
- Provider toggles layer per folder. The nearest `.foac.toml` from the working
  directory up to `/` overrides the global toggles via its `enabled_providers`
  and `disabled_providers` arrays (a name in both is enabled). Entries are
  bare provider names (the whole provider) or qualified `provider@instance`
  names (one instance); an instance is active iff its provider is enabled and
  its qualified name is not disabled.
  `foac provider <enable|disable> <name> [--instance <name>] --local` edits
  that nearest file (creating one in the working directory when none exists)
  with the same comment-preserving TOML machinery as the global config;
  credentials are shared and untouched by toggling. The layering lives in
  `provider.rs`: settings loads attach the local overrides, so every consumer
  of `Settings::enabled` sees the effective state.
- Inactive providers are invisible. Unauthenticated or disabled providers
  are hidden from `--help`, from suggestions, and from the rendered skill, but
  their commands still parse and their `--help` still works. Auth probing is
  expensive (config reads, a `gh` subprocess), so it only happens on the
  error/help path, never on the hot parse path.
- Commands never print success output. They return response JSON to their
  `run`, which prints once through `output.rs`. Table rendering is a heuristic
  over JSON shapes in one place, never per-command formatting.
- On Linear mutations, omitted means untouched. Unset optional fields are
  omitted from GraphQL variables (`skip_serializing_none`), never sent as
  `null`, because null wipes the field server-side. There is a test asserting
  this. Keep it true for every new mutation.
- `assets/SKILL.md` moves with the CLI surface. It is compiled into the
  binary and rendered into one skill per provider (`foac-<provider>`): marker
  comments select each provider's frontmatter and sections around the shared
  content. `foac skill install` writes the active providers' skills into
  agents' skill folders and removes inactive ones. CI validates every rendered
  skill. Any change to commands, flags, or conventions updates the source file
  in the same commit.
- Releases are automated. A push to main touching `src/`, `assets/`, or
  Cargo files bumps the version from conventional-commit prefixes and
  publishes binaries. Commit prefixes are therefore load-bearing.

## Data flow

### An agent lists Linear issues

1. `foac linear issue list --team ENG --format json` → `main.rs` parses (plain
   parse, no auth probe) → resolves the instance (flag > `[defaults]` >
   `default`) → `provider::ensure_enabled` → `linear::run`.
2. `linear.rs` builds the compile-time-checked `IssueList` query; `eq_filter!`
   turns `ENG` into a key filter (a UUID would become an id filter). Token
   from `auth::linear_token` for the resolved instance (env, else credentials
   file, for `default`; stored only for a named instance).
3. Response `data` JSON returns to `run` → `output::print` → compact JSON on
   stdout (a TTY without `--format json` would get a table). Errors: provider
   JSON on stderr, exit 1.

### A human checks auth

1. `foac auth status` → `auth.rs` resolves each provider's credential, calls
   each provider's `auth_identity` for live validation.
2. Per-provider status objects (`authenticated`/`unauthenticated`/`error`,
   source, safe account fields only) → printed as JSON, or flattened to a
   table on a TTY. Exit 0 regardless; callers inspect the JSON.

## Where this system is meant to grow

- More providers. Candidates are tracked in
  [GitHub issues](https://github.com/alephic-ai/foac/issues). A new REST provider
  follows `sentry.rs`'s shape on top of the shared core in `rest.rs`; a new
  GraphQL provider copies Linear's setup: vendored schema,
  `queries.graphql`, codegen macro. The full recipe, including every
  registration and documentation touch point, is
  `doc/adding-a-provider.md`.
- Structural change is provider-driven. When adding a provider raises a
  question the current structure can't answer, restructure then. Don't add
  speculative abstraction beforehand, and don't work around the gap. The
  same applies to the flat one-file-per-provider layout: it stays until it is
  no longer sustainable. Don't split a provider file into a module directory
  preemptively.
- Deeper coverage of existing providers (new resources/verbs) follows
  `doc/adding-a-command.md`.
- Binary transfer and log streaming are on the backlog. Add them if they can
  be made explicit and discoverable in the command grammar (today's commands
  are JSON metadata only).

## Development

```sh
cargo run -- --help     # run
cargo test --locked     # test (unit tests inline, e2e in tests/cli.rs)
```

Conventions: read `doc/layout.md` before touching `src/`;
`doc/adding-a-command.md` for new commands; `doc/releasing.md` for the
release flow. Tests must not hit real auth: parse-clean commands only in
`tests/cli.rs`, injected stores/local TCP servers elsewhere.
