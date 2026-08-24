# foac

foac, the Father Of All CLIs, wraps external SaaS APIs (Linear, GitHub, Sentry,
Slack, more to come) behind one command grammar:
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
   ┌────────────┬────────────┬────────────┬────────────┬─────────────┐
   │ linear.rs  │ github.rs  │ sentry.rs  │ slack.rs   │ auth.rs     │
   │ (GraphQL,  │ (REST,     │ (REST,     │ (REST,     │ provider.rs │
   │  codegen)  │  untyped)  │  untyped)  │  untyped)  │ update.rs   │
   └─────┬──────┴─────┬──────┴─────┬──────┴─────┬──────┴─────────────┘
         └────────────┴────────────┴──────┬──────┘
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
├── github.rs    # GitHub provider: REST passthrough (also hosts shared REST helpers, see drift note)
├── sentry.rs    # Sentry provider: REST passthrough, same pattern as github.rs
├── slack.rs     # Slack provider: REST with Slack's HTTP-200 ok/error envelope
├── auth.rs      # Credential resolution (env > credentials file > gh CLI), validation, auth commands
├── provider.rs  # Comment-preserving TOML settings + private JSON credentials
├── output.rs    # The one success printer: compact JSON, or shape-heuristic tables on a TTY
├── update.rs    # Self-update from GitHub Releases + once-a-day version check
└── lib.rs       # Library target so tests/ and doc tests can link; main.rs is the only consumer
graphql/linear/  # Vendored schema (51k lines; grep it) + queries.graphql (compile-time checked)
doc/SKILL.md     # Agent skill, compiled into the binary; must track every CLI surface change
```

Known drift: `sentry.rs` imports `insert_opt`/`push_query` from `github.rs`,
while Sentry and Slack each retain provider-specific request plumbing. Do not
copy this again. The next REST provider is the trigger to extract a shared
REST core module.

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
- Auth is a proxy, resolved per provider as env var > credentials file (> `gh`
  CLI for GitHub). Log in once, reuse everywhere. Credentials live in the
  atomically replaced `credentials.json`, which is mode 0600 before secret
  bytes are written on Unix, not the OS keychain (rebuilt binaries lose macOS
  Keychain ACL trust, #35). Tokens are never printed. Login validates before
  storing.
  Slack stores bot and user credentials independently. Ordinary commands resolve
  bot env > stored bot > user env > stored user; search resolves user env >
  stored user because Slack does not allow bot tokens to call `search.messages`.
  Slack login asks for both and validates the pair before one atomic credential
  write. This makes bot-only, user-only, both-token, and unauthenticated
  installations explicit capability modes.
- Settings and credentials fail closed independently. Provider toggles and the
  optional Sentry URL live in comment-preserving `config.toml`; credentials
  for every provider live in pretty-printed `credentials.json`. Unknown TOML
  keys are discarded on write. Legacy `config.json` is a deliberately ignored
  fresh-start format: it is never read, migrated, deleted, or used as fallback.
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
- `doc/SKILL.md` moves with the CLI surface. It is compiled into the
  binary and rendered into one skill per provider (`foac-<provider>`): marker
  comments select each provider's frontmatter and sections around the shared
  content. `foac skill install` writes the active providers' skills into
  agents' skill folders and removes inactive ones. CI validates every rendered
  skill. Any change to commands, flags, or conventions updates the source file
  in the same commit.
- Releases are automated. A push to master touching `src/`, `graphql/`,
  `doc/SKILL.md`, or Cargo files bumps the version from conventional-commit
  prefixes and publishes binaries. Commit prefixes are therefore load-bearing.

## Data flow

### An agent lists Linear issues

1. `foac linear issue list --team ENG --format json` → `main.rs` parses (plain
   parse, no auth probe) → `provider::ensure_enabled` → `linear::run`.
2. `linear.rs` builds the compile-time-checked `IssueList` query; `eq_filter!`
   turns `ENG` into a key filter (a UUID would become an id filter). Token
   from `auth::linear_token` (env, else credentials file).
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

- Per-project provider toggles. Today enable/disable lives only in the global
  settings file; the promise is that each project can enable a different set of
  providers, with credentials always shared and untouched by toggling. The
  project-level toggle layer goes in `provider.rs` on top of the global one.
- More providers. Candidates are tracked in
  [GitHub issues](https://github.com/lra/foac/issues). A new REST provider
  follows `sentry.rs`'s shape but first extracts the shared REST core out of
  the existing REST providers. The core owns any shared boilerplate: the
  `Api` struct, list wrapping, payload helpers, and the hand-rolled
  auth-identity HTTP each provider duplicates today. Write the REST
  adding-a-command recipe down at the same time. A new GraphQL provider copies
  Linear's setup: vendored schema, `queries.graphql`, codegen macro. Every
  provider addition also touches: `auth.rs` (Provider enum, token resolution, identity),
  `provider.rs`'s `PROVIDERS`, `main.rs`'s `providers()`, and `doc/SKILL.md`
  (with marker blocks, including a frontmatter `name`/`description` pair and
  an H1 for the new provider).
- Structural change is provider-driven. When adding a provider raises a
  question the current structure can't answer, restructure then. Don't add
  speculative abstraction beforehand, and don't work around the gap. The
  same applies to the flat one-file-per-provider layout: it stays until it is
  no longer sustainable. Don't split a provider file into a module directory
  preemptively.
- Deeper coverage of existing providers (new resources/verbs) follows
  `doc/adding-a-command.md` for Linear; REST resources follow any existing
  resource in the provider's file.
- Binary transfer and log streaming are on the backlog. Add them if they can
  be made explicit and discoverable in the command grammar (today's commands
  are JSON metadata only).

## Development

```sh
cargo run -- --help     # run
cargo test --locked     # test (unit tests inline, e2e in tests/cli.rs)
```

Conventions: read `doc/layout.md` before touching `src/`;
`doc/adding-a-command.md` for new Linear commands; `doc/releasing.md` for the
release flow. Tests must not hit real auth: parse-clean commands only in
`tests/cli.rs`, injected stores/local TCP servers elsewhere.
