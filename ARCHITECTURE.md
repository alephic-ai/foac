# foac

> foac, the Father Of All CLIs: one consistent command grammar —
> `foac <provider> <resource> <verb>` — wrapping external SaaS APIs (Linear,
> GitHub, Sentry, Slack, more to come). The primary consumer is an LLM agent
> working in a shell; humans at a TTY are served by a rendering layer on top of
> the same output. foac's job is to make any provider's API discoverable, uniform,
> and already authenticated — not to abstract or improve it.

## How the System Maps to the World

Each first-level subcommand is a **provider** — an external product's API.
Resources are the provider's own nouns (`issue`, `pull`, `release`), verbs are
`list`/`get`/`create`/`update`/`delete`. The compiled-in clap command tree
(plus, for Linear, the vendored GraphQL schema) is a **local cache of each
provider's API structure**: an agent discovers the API through `--help` with
no network round-trip, which keeps discovery deterministic, fast, and
token-cheap. Response bodies are the provider's raw JSON — foac never lies
about what the API returned, so upstream API docs remain valid documentation
for foac's output.

Three cross-provider concerns are foac's own: **auth** (log in once, every
agent harness and script reuses the credential), **discovery** (help text and
a self-installing agent skill that only show authenticated, enabled
providers), and **output** (JSON for machines, tables for humans, decided per
invocation).

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
(`auth.rs`) and the config file (`provider.rs`, `~/.config/foac/config.json`,
mode 0600: credentials, provider toggles, Sentry host) sit under all
providers.

## Codemap

```text
src/
├── main.rs      # CLI root: dispatch, hiding inactive providers, skill render/install
├── linear.rs    # Linear provider — GraphQL via graphql_client codegen
├── github.rs    # GitHub provider — REST passthrough (also hosts shared REST helpers, see drift note)
├── sentry.rs    # Sentry provider — REST passthrough, same pattern as github.rs
├── slack.rs     # Slack provider — REST with Slack's HTTP-200 ok/error envelope
├── auth.rs      # Credential resolution (env > config file > gh CLI), live validation, auth commands
├── provider.rs  # The config file: enable/disable toggles + stored credentials
├── output.rs    # The one success printer: compact JSON, or shape-heuristic tables on a TTY
├── update.rs    # Self-update from GitHub Releases + once-a-day version check
└── lib.rs       # Library target so tests/ and doc tests can link; main.rs is the only consumer
graphql/linear/  # Vendored schema (51k lines — grep it) + queries.graphql (compile-time checked)
doc/SKILL.md     # Agent skill, compiled into the binary; must track every CLI surface change
```

Known drift: `sentry.rs` imports `insert_opt`/`push_query` from `github.rs`,
while Sentry and Slack each retain provider-specific request plumbing. Do not
copy this again — the next REST provider is the trigger to extract a shared
REST core module.

## Key Decisions

- **Raw JSON passthrough** — each provider is assumed to use the data shape
  most adequate for its domain, so foac forwards responses as-is; agents can
  trust the fields against upstream docs. The one standing normalization is
  wrapping REST lists as `{"items": [...], "pageInfo": {...}}`; further
  reshaping is allowed only when it clearly earns its keep (e.g. GitHub issue
  lists filter out pull requests).
- **The command tree is a compiled-in API cache** — `--help` at every level is
  the ground truth, generated from clap definitions checked at compile time
  (and, for Linear, queries validated against the vendored schema). Discovery
  must stay offline and deterministic; never make what a command accepts
  depend on runtime state.
- **Uniform grammar across providers** — provider/resource/verb, `--limit` +
  cursor-or-page flags, `{items, pageInfo}` lists, JSON errors on stderr with
  exit 1. Shaping one provider differently from the others breaks
  deterministic discovery; this uniformity is the bet that keeps foac fast to
  use with many providers enabled.
- **Codegen only when the provider publishes a schema** — Linear (GraphQL)
  gets compile-time-checked queries via `graphql_client`; REST providers stay
  untyped passthrough. The less to maintain in foac, the better; don't
  hand-write types for REST responses.
- **Auth is a proxy, resolved per provider as env var > config file (> `gh`
  CLI for GitHub)** — log in once, reuse everywhere. Credentials live in the
  0600 config file, not the OS keychain (rebuilt binaries lose macOS Keychain
  ACL trust, #35). Tokens are never printed; login validates before storing.
  Slack stores bot and user credentials independently. Ordinary commands resolve
  bot env > stored bot > user env > stored user; search resolves user env >
  stored user because Slack does not allow bot tokens to call `search.messages`.
  Slack login asks for both and validates the pair before one atomic config
  write. This makes bot-only, user-only, both-token, and unauthenticated
  installations explicit capability modes.
- **Inactive providers are invisible** — unauthenticated or disabled providers
  are hidden from `--help`, from suggestions, and from the rendered skill, but
  their commands still parse and their `--help` still works. Auth probing is
  expensive (config reads, a `gh` subprocess), so it only happens on the
  error/help path — never on the hot parse path.
- **Commands never print success output** — they return response JSON to their
  `run`, which prints once through `output.rs`. Table rendering is a heuristic
  over JSON shapes in one place, never per-command formatting.
- **On Linear mutations, omitted means untouched** — unset optional fields are
  omitted from GraphQL variables (`skip_serializing_none`), never sent as
  `null`, because null wipes the field server-side. There is a test asserting
  this; keep it true for every new mutation.
- **`doc/SKILL.md` moves with the CLI surface** — it is compiled into the
  binary and rendered into one skill per provider (`foac-<provider>`): marker
  comments select each provider's frontmatter and sections around the shared
  content. `foac skill install` writes the active providers' skills into
  agents' skill folders and removes inactive ones; CI validates every rendered
  skill. Any change to commands, flags, or conventions updates the source file
  in the same commit.
- **Releases are hands-off** — a push to master touching `src/`, `graphql/`,
  `doc/SKILL.md`, or Cargo files bumps the version from conventional-commit
  prefixes and publishes binaries. Commit prefixes are therefore load-bearing.

## Data Flow

### An agent lists Linear issues

1. `foac linear issue list --team ENG --format json` → `main.rs` parses (plain
   parse, no auth probe) → `provider::ensure_enabled` → `linear::run`.
2. `linear.rs` builds the compile-time-checked `IssueList` query; `eq_filter!`
   turns `ENG` into a key filter (a UUID would become an id filter). Token
   from `auth::linear_token` (env, else config file).
3. Response `data` JSON returns to `run` → `output::print` → compact JSON on
   stdout (a TTY without `--format json` would get a table). Errors: provider
   JSON on stderr, exit 1.

### A human checks auth

1. `foac auth status` → `auth.rs` resolves each provider's credential, calls
   each provider's `auth_identity` for live validation.
2. Per-provider status objects (`authenticated`/`unauthenticated`/`error`,
   source, safe account fields only) → printed as JSON, or flattened to a
   table on a TTY. Exit 0 regardless; callers inspect the JSON.

## Where This System Is Meant to Grow

- **More providers.** That is the roadmap; candidates are tracked in
  [GitHub issues](https://github.com/lra/foac/issues). A new REST provider
  follows `sentry.rs`'s shape but first extracts the shared REST core out of
  the existing REST providers — the core owns any shared boilerplate: the
  `Api` struct, list wrapping, payload helpers, and the hand-rolled
  auth-identity HTTP each provider duplicates today. Write the REST
  adding-a-command recipe down at the same time. A new GraphQL provider copies
  Linear's setup: vendored schema, `queries.graphql`, codegen macro. Every
  provider addition also touches: `auth.rs` (Provider enum, token resolution, identity),
  `provider.rs`'s `PROVIDERS`, `main.rs`'s `providers()`, and `doc/SKILL.md`
  (with marker blocks, including a frontmatter `name`/`description` pair and
  an H1 for the new provider).
- **Structural change is provider-driven.** When adding a provider raises a
  question the current structure can't answer, that's the moment to restructure
  — not before (no speculative abstraction), not by working around it. The
  same applies to the flat one-file-per-provider layout: it stays until it is
  no longer sustainable; don't split a provider file into a module directory
  preemptively.
- **Deeper coverage of existing providers** (new resources/verbs) follows
  `doc/adding-a-command.md` for Linear; REST resources follow any existing
  resource in the provider's file.
- **Binary transfer and log streaming are backlog, not forbidden** — accepted
  if they can be made explicit and discoverable in the command grammar
  (today's commands are JSON-metadata only).

## Development

```sh
cargo run -- --help     # run
cargo test --locked     # test (unit tests inline, e2e in tests/cli.rs)
```

Conventions: read `doc/layout.md` before touching `src/`;
`doc/adding-a-command.md` for new Linear commands; `doc/releasing.md` for the
release flow. Tests must not hit real auth: parse-clean commands only in
`tests/cli.rs`, injected stores/local TCP servers elsewhere.
