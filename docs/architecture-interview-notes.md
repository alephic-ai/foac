# Architecture interview notes — foac

Working notes for building ARCHITECTURE.md. Investigation first, then interview Q&A.

## Investigation findings

### What foac is

"Father Of All CLIs": one consistent CLI (`foac <provider> <resource> <verb>`)
wrapping external SaaS APIs — Linear (GraphQL), GitHub (REST), Sentry (REST).
Rust, ~7k lines, single binary, GPL-3.0, released via GitHub Releases and
installed with ubi. Author: Laurent Raufaste.

Strong signals the primary consumer is an LLM agent, not a human:

- Compact JSON on stdout, errors as JSON on stderr, exit 1, nothing interactive
  (auth login is the sole documented exception).
- `doc/SKILL.md` is compiled into the binary (`include_str!`), rendered per
  active provider, printed by `foac skill print`, installed into agent skill
  folders (`~/.claude/skills`, `~/.agents/skills`). CI validates it with skref.
- Table output for humans came later (v1.4, #34) as a TTY-only rendering of the
  same JSON, via shape heuristics in `output.rs` — not per-command table code.

### Module map (flat, one file per concern)

- `main.rs` (499) — clap parse, dispatch, provider hiding, skill render/install.
  Two-phase parse: plain parse first; only on error/help re-parse with the
  provider-hiding command, because building it probes auth (config reads, `gh`
  subprocess) — deliberately kept off the hot path (#31 follow-up).
- `linear.rs` (1358) — GraphQL. `graphql_client` codegen at compile time against
  vendored 51k-line schema; queries in `graphql/linear/queries.graphql`;
  `linear_query!` macro registers each op. `eq_filter!` lets list filters accept
  UUID or human name; mutations require UUIDs.
- `github.rs` (2283) — REST passthrough. `Api` struct (client/base_url/token/format),
  `path!` macro, `ListShape` enum (Array/Key/Issues) to normalize list bodies,
  Link-header pagination → `{items, pageInfo}`. Repo resolution from git remotes.
- `sentry.rs` (621) — same REST pattern as github.rs. **Imports `insert_opt` and
  `push_query` from `crate::github`**; duplicates the `Api` struct rather than
  sharing it. Trailing-slash URLs required. Cursor pagination from Link header.
  Short-ID → numeric ID resolution via an extra API call.
- `auth.rs` (1049) — credential resolution + live validation for all providers.
  `SecretStore` trait, one real impl (`ConfigFileStore`) + test `MemoryStore`.
  Precedence: env var > config file (> `gh auth token` for GitHub only).
  Status JSON: `authenticated | unauthenticated | error` + source + safe account.
- `provider.rs` (262) — the config file `~/.config/foac/config.json` (0600):
  disabled_providers, credentials, sentry_url. Enable/disable + `ensure_enabled`.
- `output.rs` (444) — shared success printer. JSON compact, or table on TTY.
  Table rendering is heuristic on JSON shape: GitHub `{items,pageInfo}`,
  Linear connection (`nodes`+`pageInfo`, depth 1 or 2), keyed maps, single
  objects. `nodes` without sibling `pageInfo` is a relation, not a list.
- `update.rs` (341) — `foac update` (self_update from GitHub Releases) + daily
  update check with a cache file, semver-compared so dev builds aren't nagged.
- `lib.rs` — lib target only so `tests/` and doc tests can link (#38).

### Conventions observed

- Every command returns response JSON to `run`, which prints once through
  `output::print`. Commands never print success output themselves.
- Raw API JSON passthrough is the invariant; only lists get wrapped/normalized.
- Unset optional GraphQL fields are omitted, never null (null wipes data
  server-side on Linear updates) — `skip_serializing_none` + a test asserting it.
- Pure-function extraction for testability everywhere: `resolve()`, `check()`,
  `config_path()`, `render_inner()` take injected env/tty/time params.
- Tests use hand-rolled single-request TCP servers, no mock crates.
- 8 deps total, all boring. `reqwest` blocking, rustls.
- Conventional commits drive automated release (release.yml bumps version from
  prefixes; works off tip of master; drafts count as unreleased and resume).
- CLAUDE.md → symlink to AGENTS.md (single source).

### Decision rationale recovered from git history (no need to ask)

- Keychain → config file (#35, feat!): rebuilt binaries lose macOS Keychain ACL
  trust, prompting for password on every new build. 0600 config.json instead.
- Hidden providers (#31): unauthenticated/disabled providers hidden from help,
  suggestions, and the rendered skill; auth probing kept off the hot parse path.
- Table output (#34): humans get tables at a TTY, agents get JSON; shared
  printer instead of per-command rendering.
- `/releases/latest` used for update (never returns drafts, unlike the listing
  self_update walks); asset_identifier required because .sha256 sorts first.

### Contradictions / gaps to probe in the interview

1. `doc/adding-a-command.md` covers only Linear. GitHub/Sentry (REST) have no
   recipe, yet Sentry was added by following github.rs.
2. Shared REST helpers (`insert_opt`, `push_query`) live in `github.rs` and are
   imported by `sentry.rs`; the `Api` struct is duplicated between the two.
   Deliberate rule-of-three laziness, or drift waiting for a cleanup?
3. Naming: `provider.rs` is really the config file + enable/disable; `auth.rs`
   also reads/writes that config. Boundary between the two is by feature, not
   by data ownership.

### Questions only the author can answer

- Q1: metaphor / primary user (agent-first?) and what "Father Of All CLIs" means
  for scope — what belongs in foac vs not (e.g. `gh`, `sentry-cli` exist).
- Q2: raw JSON passthrough vs a normalized cross-provider schema — why
  passthrough, and is the list wrapper the only normalization ever allowed?
- Q3: per-provider single file — hard rule? What happens when github.rs keeps
  growing? When does the shared REST core get extracted?
- Q4: would a new GraphQL provider copy Linear's codegen approach, and a new
  REST provider copy sentry.rs? What's the recipe in his head?
- Q5: what's next (more providers? more resources? binary transfer?) and what
  would a new engineer/agent most likely get wrong?

## Interview log

- Q: github→sentry REST duplication (`Api` struct, helpers in github.rs) —
  deliberate rule-of-three deferral or drift?
  A: **Drift.** The author does not want it duplicated a third time: the next
  REST provider is the moment to extract a shared REST core, not to copy
  sentry.rs again. `insert_opt`/`push_query` living in github.rs is part of the
  same drift.
- Q: is raw JSON passthrough load-bearing, and is the `{items, pageInfo}`
  wrapper the only allowed reshaping?
  A: Load-bearing: **foac never lies about what the API returned** — agents can
  consult upstream provider docs and trust the fields. Each provider is assumed
  to use the data structure most adequate/optimal for its domain, so foac
  forwards it as-is; the consumer (usually an agent) can deal with different
  formats. The goal is a *discoverable* way for the agent to get the data it
  needs. The wrapper is NOT the only reshaping ever allowed — more may make
  sense in the future — but reshaping is the exception, forwarding the rule.
- Q: what earns a provider/command a place in foac (why wrap GitHub when `gh`
  exists; is "JSON metadata only" a principle)?
  A: Value proposition = uniform grammar + guaranteed JSON + self-installing
  skill, **plus the auth proxy**: auth into foac once, reuse in every agent
  harness or script without re-authing each agent. Less compelling against a
  good CLI like `gh`, but many providers have bad CLIs or none (MCP/API only) —
  that's where foac shines; GitHub coverage still pays via discoverability
  (less work, fewer tokens for the agent). "Metadata only" is **backlog, not
  principle** — start with the basics; binary/stream support is acceptable if
  it can be made explicit and discoverable in the command structure.
- Q: is the rule "codegen when the provider publishes a schema (GraphQL),
  untyped passthrough when it doesn't (REST)"?
  A: **Yes, that's it** — the less to maintain in foac, the better. But a key
  reframe: foac still wants to *compile in the structure of each provider's
  API*. The clap command tree (and the vendored GraphQL schema) act as a
  **local cache of the API structure**, so agent discovery (`--help`) happens
  locally: faster and cheaper (no network, fewer tokens).
- Q: what's next, and what would a newcomer most likely get wrong?
  A: Next = **more providers**; structural change happens when a new provider
  raises questions (provider-driven evolution — matches the REST-core
  extraction being deferred to the next REST provider). Traps confirmed:
  SKILL.md drift, printing success output inside a command, null-vs-omitted on
  Linear updates. Plus the big one: **shaping one provider differently from the
  others** — non-uniform shapes make discovery non-deterministic; determinism
  of the grammar is why foac should stay fast to use/discover even with many
  providers enabled.

## Draft review

- Overview confirmed: agent-first, humans via rendering layer. Update/skill
  machinery are "the how", not part of the pitch; the "why" is authenticate
  once + local-cache API discovery.
- Key Decisions (all 10) confirmed as-is, nothing missing.
- Open questions resolved during review: the future REST core owns *any*
  shared boilerplate (including auth-identity HTTP); the flat file-per-provider
  layout stays until unsustainable; the REST recipe gets written down when the
  core is extracted. Provider candidates are tracked in
  <https://github.com/lra/foac/issues>.
- New open question from the author: per-provider skills instead of the single
  filtered SKILL.md — to be evaluated.

## Remaining gaps

None significant. The author held strong, consistent theory across all three
tracks; the document stabilized in one review round. Weakest documented area
going in was the REST-provider recipe (code-only), now flagged in the growth
section to be written when the REST core is extracted.
