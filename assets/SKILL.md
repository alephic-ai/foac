---
<!-- foac-provider:airtable -->
name: foac-airtable
description: Use the foac CLI to interact with Airtable from the shell. Covers bases, table schemas (fields and views), records, and record comments.
<!-- /foac-provider:airtable -->
<!-- foac-provider:axiom -->
name: foac-axiom
description: Use the foac CLI to interact with Axiom from the shell. Covers datasets, fields, APL queries, event ingestion, annotations, monitors, notifiers, users, and organizations.
<!-- /foac-provider:axiom -->
<!-- foac-provider:confluence -->
name: foac-confluence
description: Use the foac CLI to interact with Confluence from the shell. Covers spaces, pages, footer comments, and CQL search.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
name: foac-firecrawl
description: Use the foac CLI to interact with Firecrawl from the shell. Covers web scraping, site maps, web search, crawl jobs, batch scrapes, browsing agents, and team usage. Also use as the retry when a plain fetch of a public URL fails or returns unusable content — 403, bot check, paywall-shaped block, or a JS-rendered page — instead of retrying with curl.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:fireflies -->
name: foac-fireflies
description: Use the foac CLI to interact with Fireflies.ai from the shell. Covers meeting transcripts, summaries and action items, AskFred questions about meetings, bites, channels, contacts, and users.
<!-- /foac-provider:fireflies -->
<!-- foac-provider:github -->
name: foac-github
description: Use the foac CLI to interact with GitHub from the shell. Covers repositories, issues, pull requests, reviews, Actions, branches, commits, checks, releases, labels, artifacts, and collaborators.
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
name: foac-jira
description: Use the foac CLI to interact with Jira from the shell. Covers issues, comments, projects, sprints, users, and workflow transitions.
<!-- /foac-provider:jira -->
<!-- foac-provider:linear -->
name: foac-linear
description: Use the foac CLI to interact with Linear from the shell. Covers issues, projects, teams, users, cycles, labels, workflow states, documents, initiatives, milestones, status updates, attachments, and issue relations (blocking dependencies).
<!-- /foac-provider:linear -->
<!-- foac-provider:neon -->
name: foac-neon
description: Use the foac CLI to interact with Neon from the shell. Covers organizations, projects, branches, databases, roles, compute endpoints, operations, and connection URIs.
<!-- /foac-provider:neon -->
<!-- foac-provider:notion -->
name: foac-notion
description: Use the foac CLI to interact with Notion from the shell. Covers pages and their Markdown content, databases, data sources and their rows, blocks, comments, search, and users.
<!-- /foac-provider:notion -->
<!-- foac-provider:sentry -->
name: foac-sentry
description: Use the foac CLI to interact with Sentry from the shell. Covers organizations, projects, issues, error events, and releases.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
name: foac-slack
description: Use the foac CLI to interact with Slack from the shell. Covers conversations, messages, threads, users, message search, and reactions.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
name: foac-vercel
description: Use the foac CLI to interact with Vercel from the shell. Covers teams, projects, deployments, account domains, and project domains.
<!-- /foac-provider:vercel -->
---

<!-- rumdl-disable MD022 MD025 -->
<!-- foac-provider:airtable -->
# foac-airtable
<!-- /foac-provider:airtable -->
<!-- foac-provider:axiom -->
# foac-axiom
<!-- /foac-provider:axiom -->
<!-- foac-provider:confluence -->
# foac-confluence
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
# foac-firecrawl
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:fireflies -->
# foac-fireflies
<!-- /foac-provider:fireflies -->
<!-- foac-provider:github -->
# foac-github
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
# foac-jira
<!-- /foac-provider:jira -->
<!-- foac-provider:linear -->
# foac-linear
<!-- /foac-provider:linear -->
<!-- foac-provider:neon -->
# foac-neon
<!-- /foac-provider:neon -->
<!-- foac-provider:notion -->
# foac-notion
<!-- /foac-provider:notion -->
<!-- foac-provider:sentry -->
# foac-sentry
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
# foac-slack
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
# foac-vercel
<!-- /foac-provider:vercel -->
<!-- rumdl-enable MD022 MD025 -->

foac wraps external provider APIs as CLI subcommands for LLM agents: every
command prints compact JSON on stdout (pass `--format json` when parsing; a
human at an interactive terminal gets a table instead), errors go to stderr
with exit code 1, and provider API commands are non-interactive. Auth login is
the explicit exception: it securely prompts on a TTY or reads a token from
redirected stdin.

## Structure

```text
foac <provider> <resource> <verb> [flags]
```

- A provider is the external product or API named by the first command segment.
  The top-level `--help` lists only authenticated, enabled providers, under a
  separate `Providers:` heading; `foac auth --help` lists every provider
  there, since logging in is how one becomes active.
<!-- foac-provider:airtable -->
- `airtable`: bases, table schemas, records, and record comments.
<!-- /foac-provider:airtable -->
<!-- foac-provider:axiom -->
- `axiom`: datasets, fields, APL queries, event ingestion, annotations,
  monitors, notifiers, users, and organizations.
<!-- /foac-provider:axiom -->
<!-- foac-provider:linear -->
- `linear`: issues, projects, teams, users, cycles, labels, workflow states,
  documents, initiatives, milestones, status updates, attachments, and issue
  relations (blocking dependencies).
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- `github`: repositories, issues, pull requests, reviews, Actions, branches,
  commits, checks, releases, labels, artifacts, and collaborators.
<!-- /foac-provider:github -->
<!-- foac-provider:confluence -->
- `confluence`: spaces, pages, footer comments, and CQL search.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
- `firecrawl`: web scraping, site maps, web search, crawl jobs, batch
  scrapes, browsing agents, and team usage.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:fireflies -->
- `fireflies`: meeting transcripts, summaries, AskFred questions, bites,
  channels, contacts, and users.
<!-- /foac-provider:fireflies -->
<!-- foac-provider:jira -->
- `jira`: issues, comments, projects, sprints, users, and workflow
  transitions.
<!-- /foac-provider:jira -->
<!-- foac-provider:neon -->
- `neon`: organizations, projects, branches, databases, roles, compute
  endpoints, operations, and connection URIs.
<!-- /foac-provider:neon -->
<!-- foac-provider:notion -->
- `notion`: pages and their Markdown content, databases, data sources and
  their rows, blocks, comments, search, and users.
<!-- /foac-provider:notion -->
<!-- foac-provider:sentry -->
- `sentry`: organizations, projects, issues, error events, and releases.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- `slack`: conversations, messages, threads, users, message search, and
  reactions.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
- `vercel`: teams, projects, deployments, account domains, and project domains.
<!-- /foac-provider:vercel -->
- Resources are nouns (`issue`, `project`, `team`, `user`, ...), verbs are
  `list`, `get`, `create`, `update`, `delete`.
- `--help` at any level lists what exists and which flags each verb takes.
  Explore with `foac <provider> --help`, then
  `foac <provider> <resource> --help`.

## Conventions

- **Auth commands**: Use `foac auth status` for all providers, or
  `foac auth <provider> <status|login|logout>` for one. Login securely reads and
  validates a token before saving it in foac's credentials file; when stdin is
  redirected, it reads the token from stdin. Logout removes only foac's stored
  credential. Use `foac auth --help` to list auth targets.
- **Instances**: a provider can hold several named logins to different
  tenants (e.g. two Slack workspaces). `foac auth <provider> login --instance
  <name>` stores one; commands select it with the global `-i`/`--instance`
  flag, else the nearest `.foac.toml` (or global config) `[defaults]` table
  (`slack = "workb"`), else the `default`
  instance — which is the unnamed login and behaves exactly as before.
  Environment tokens and the `gh` fallback apply to the default instance
  only; a named instance uses exactly its stored credentials. Instance names
  are lowercase letters, digits, `-`, `_`.
- **Provider toggles**: `foac provider <enable|disable> <name>` turns a provider
  on or off (state kept in `~/.config/foac/config.toml`) and prints the same
  per-provider map as `foac provider list`, which reports each provider's
  `enabled`, `authenticated` (a credential resolves; not validated against the
  API), and `skill_installed` state, plus one `provider@instance` entry per
  stored named instance. Add `--instance <name>` to toggle a single instance
  (stored as `provider@instance` in the same arrays) instead of the whole
  provider. Disabled providers and instances are hidden from discovery and
  their commands refuse to run. A `.foac.toml` file
  with `enabled_providers` and/or `disabled_providers` string arrays, found in
  the working directory or the nearest parent, overrides the global toggles
  for that folder tree. Add `--local` to enable/disable to write the toggle to
  that nearest `.foac.toml` instead (created in the working directory if none
  exists).
- **Storage**: Credentials are pretty-printed in
  `~/.config/foac/credentials.json` (nested provider → instance → fields),
  atomically replaced, and mode `0600`
  before secret bytes are written on Unix. Settings use comment-preserving
  TOML. Missing files are valid first-run state; malformed stores fail closed
  independently with their path and cause.
<!-- foac-provider:airtable -->
- **Airtable auth precedence**: `AIRTABLE_API_KEY`, then the credentials
  file. The token is a personal access token; it only sees the bases it was
  granted, and only within its scopes (`data.records:read`/`write`,
  `data.recordComments:read`/`write`, `schema.bases:read`). A base or table
  the token cannot reach, or a missing scope, answers 403
  `INVALID_PERMISSIONS_OR_MODEL_NOT_FOUND`, not 401.
<!-- /foac-provider:airtable -->
<!-- foac-provider:axiom -->
- **Axiom auth precedence**: `AXIOM_TOKEN`, then the credentials file. An
  API token (`xaat-`) carries its own organization; a personal access token
  (`xapt-`) also needs the organization ID: `--org-id` anywhere after
  `axiom`, `AXIOM_ORG_ID` (default instance only), or the ID saved with the
  token by `foac auth axiom login --org-id ID`. The API base URL comes from
  `AXIOM_URL` (default instance only), then the host saved by
  `foac auth axiom login --host HOST` for a regional or dedicated deployment.
<!-- /foac-provider:axiom -->
<!-- foac-provider:linear -->
- **Linear auth precedence**: `LINEAR_API_KEY`, then the credentials file.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- **GitHub auth precedence**: `GITHUB_TOKEN`, then the credentials file, then
  `gh auth token`.
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
- **Jira auth**: every command needs an Atlassian host, email, and API token.
  Each resolves independently: `--host`/`--email` flags, then
  `ATLASSIAN_HOST`/`ATLASSIAN_EMAIL`/`ATLASSIAN_API_TOKEN`, then the
  `atlassian` credentials saved by `foac auth jira login` (which prompts for
  all three, or reads one line per missing value from redirected stdin in
  host, email, token order). A token that is neither in the environment nor
  stored is read from redirected stdin, so it never has to appear in shell
  history. The stored credential is shared at the Atlassian vendor level:
  logging in or out through Jira or Confluence covers both.
<!-- /foac-provider:jira -->
<!-- foac-provider:confluence -->
- **Confluence auth**: every command needs an Atlassian host, email, and API
  token. Each resolves independently: `--host`/`--email` flags, then
  `ATLASSIAN_HOST`/`ATLASSIAN_EMAIL`/`ATLASSIAN_API_TOKEN`, then the
  `atlassian` credentials saved by `foac auth confluence login` (which prompts
  for all three, or reads one line per missing value from redirected stdin in
  host, email, token order). A token that is neither in the environment nor
  stored is read from redirected stdin, so it never has to appear in shell
  history. The stored credential is shared at the Atlassian vendor level:
  logging in or out through Jira or Confluence covers both.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
- **Firecrawl auth precedence**: `FIRECRAWL_API_KEY`, then the credentials
  file. On a TTY, `foac auth firecrawl login` first asks for the host
  (default `api.firecrawl.dev`; an explicit `http://` scheme is kept, so a
  local Docker deployment works) and saves it with the instance's
  credentials; with redirected stdin it reads only the token, so pass
  `--host URL` to save a self-hosted host non-interactively. A self-hosted
  Firecrawl with authentication disabled accepts any non-empty token.
  `FIRECRAWL_API_URL` overrides the saved host for the default instance
  only.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:fireflies -->
- **Fireflies auth precedence**: `FIREFLIES_API_KEY`, then the credentials
  file.
<!-- /foac-provider:fireflies -->
<!-- foac-provider:neon -->
- **Neon auth precedence**: `NEON_API_KEY`, then the credentials file.
<!-- /foac-provider:neon -->
<!-- foac-provider:notion -->
- **Notion auth precedence**: `NOTION_API_KEY`, then the credentials file.
  The token is an internal integration secret or a personal access token; an
  integration only sees pages and databases shared with it, so a 404
  `object_not_found` usually means "not shared".
<!-- /foac-provider:notion -->
<!-- foac-provider:sentry -->
- **Sentry auth precedence**: `SENTRY_AUTH_TOKEN`, then the credentials file.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- **Slack auth capabilities**: ordinary commands prefer `SLACK_BOT_TOKEN`
  (`xoxb-`), then the bot credential in the credentials file, then
  `SLACK_USER_TOKEN` (`xoxp-`), then the stored user credential. `slack search`
  uses user env then stored user and requires `search:read`. Bot-only setups
  cannot search; user-only setups can use every command as the installing user;
  when both exist, ordinary commands run as the bot and search runs as the user.
  With neither, Slack is inactive. `foac auth slack login` prompts for bot then
  user, validates both before storing either, and allows either to be blank.
  Before prompting, it links to Slack's app management page and prints a JSON
  app manifest with the recommended bot and user scopes. Redirected input is
  two lines in the same order. Slack logout removes both.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
- **Vercel auth precedence**: `VERCEL_TOKEN`, then the credentials file.
<!-- /foac-provider:vercel -->
- **Auth status**: Status commands perform live validation and print
  `authenticated`, `unauthenticated`, or `error`, including the
  credential source and safe account identity when available. `foac auth status`
  prints an object keyed by provider, plus one `provider@instance` entry per
  stored named instance; `foac auth <provider> <status|login|logout>`
  prints a one-key map for that provider and instance (login matches status
  fields; logout reports `removed`). Agents should parse that JSON
  (`--format json`). A TTY
  shows a short summary for the single-provider commands; the all-provider
  table flattens `account` to an identity string. They exit zero after printing
  the report; inspect the JSON status values.
<!-- foac-provider:github -->
- **GitHub permissions**: classic tokens need `repo` for private repositories.
  Fine-grained tokens need Metadata read plus read or write access, as used, to
  Issues, Pull requests, Actions, Checks, Commit statuses, Contents, and
  Administration. Branch protection and collaborator changes need
  Administration write.
<!-- /foac-provider:github -->
- **Output**: the raw API response as JSON on stdout. JSON
  success output (including `auth` and `provider`) renders as a table sized to
  the terminal when stdout is an interactive TTY and `CI` is not set, so agents
  parsing stdout must pass
  `--format json` or set `FOAC_FORMAT=json`; pipes and CI always get JSON.
  `version`, `update`, and `skill` ignore `--format`.
  Every provider command's `--help` ends with an Output section naming its
  envelope, record path, `--from` join fields, and pagination paths — check
  it before parsing a response you have not seen.
  Failures print
  the API's error JSON on stderr and exit non-zero.
- **Piped joins**: pipe one command's `list` output into a `get` verb that
  takes its identifier as a positional argument (every `get` except
  `github release get`, whose selector is `--id`/`--tag`) and omit that
  argument to run one get per list item, joining on the field named by
  `--from`:
  `foac <provider> <resource> list | foac <provider> <resource> get --from
  <field>` (the two commands may target different providers; dots in the
  field reach into nested objects, e.g. `--from profile.email`). Overlap,
  membership, or "who is on both" questions are this join — not two lists
  plus a script. Piped input that
  is not JSON is one value per line, so `grep` output composes too. Successes
  stream as one JSON document per get on stdout (a TTY renders them as one
  combined table, one row per result); values the API reports as missing are
  summarized in a single stderr line (`5 of 21 not found: ...`); any other
  failure (auth, rate limit, network) prints its error JSON on stderr as it
  happens. The exit code is 0 when at least one get succeeded and the rest
  were misses, 1 when all missed or any get failed outright.
<!-- foac-provider:airtable -->
- **Airtable addressing**: records and comments need `--base APP_ID` and
  `--table TABLE`, a table ID (`tbl...`) or name; prefer IDs, which survive
  renames. `base list` gives base IDs; `table list --base APP_ID` gives the
  schema: table IDs, `fields[]` (name, type, options), and `views[]`.
- **Airtable records**: `fields` is keyed by field name and omits empty
  cells. `record list` takes `--view NAME_OR_ID`, `--formula` (Airtable
  formula, e.g. `AND({Status} = 'Todo', {Due} < TODAY())`), `--sort` as a
  JSON array of `{"field": NAME, "direction": "asc"|"desc"}`, and repeated
  `--field NAME` to fetch fewer columns. `record create` and `record update`
  take `--fields JSON` (or `--fields-file`); update only touches the given
  fields. `--typecast` converts strings to the field type and adds missing
  select options. Linked-record cells are arrays of record IDs.
- **Airtable pagination**: `record list` and `comment list` take `--limit N`
  (default 50, at most 100); `base list`, `record list`, and `comment list`
  take an opaque `--after CURSOR`.
  Output is `{"items":[...],"pageInfo":{"hasNextPage":...,"endCursor":...}}`;
  follow `pageInfo.endCursor` while `hasNextPage` is true. `table list` is
  one page.
<!-- /foac-provider:airtable -->
<!-- foac-provider:axiom -->
- **Axiom permissions**: create API tokens with Advanced permissions. On
  All datasets (or individual ones): Ingest create for `ingest`, Query read
  for `query`, Trim update for `dataset trim`. At org level: Datasets
  create/read/update/delete for `dataset` and `field`, Annotations
  create/read/update/delete for `annotation`, and read on Monitors,
  Notifiers, and Users for `monitor`, `notifier`, and `user`. Grant only
  what the commands you run need.
- **Axiom queries**: `query "APL"` runs an APL query across datasets, e.g.
  `['logs'] | where level == 'error' | summarize count() by bin(_time, 1h)`.
  Put relative ranges in the query (`| where _time > ago(1h)`) or pass
  `--start-time`/`--end-time` as RFC 3339. Output is a foac list: `items`
  holds one object per result row keyed by the query's output fields
  (Axiom's columnar tables transposed), `pageInfo` carries `hasNextPage`
  plus Axiom's `minCursor`/`maxCursor`, and `status` is Axiom's raw query
  status (`rowsMatched`, `elapsedTime`, `messages`). To page, sort by
  `_time` and pass `pageInfo.maxCursor` (ascending) or `minCursor`
  (descending) to `--cursor` while `hasNextPage` is true; it turns false
  once a page has no rows past the cursor. Use
  `field list --dataset NAME` to discover a dataset's columns first.
- **Axiom ingest**: `ingest DATASET --events JSON` or
  `--events-file PATH` (`-` for stdin) accepts a JSON array, one object, or
  NDJSON and posts them as one batch; `--timestamp-field` names the field
  holding each event's time. Datasets are identified by name everywhere.
- **Axiom lists**: management lists (`dataset`, `field`, `annotation`,
  `monitor`, `notifier`, `user`, `org`) are single-page
  `{"items":[...],"pageInfo":{"hasNextPage":false}}`. Annotations filter
  with repeatable `--dataset` plus `--start`/`--end`; `monitor history`
  requires `--start-time` and `--end-time`. Monitors and notifiers are
  read-only.
<!-- /foac-provider:axiom -->
<!-- foac-provider:linear -->
- **Linear pagination**: `list` verbs take `--limit N` (default 50) and
  `--after CURSOR`; loop using `pageInfo.endCursor` while `hasNextPage` is true.
- **Linear filters accept names**: filter flags on `list` verbs take a UUID or a
  human value: a team key (`ENG`), a state name (`In Progress`), a user
  email or display name, a project or label name.
- **Linear mutations need UUIDs**: flags on `create`/`update` verbs
  (`--assignee`, `--state`, `--project`, ...) require UUIDs. Look them up first
  with the matching `list` command. Issues are the exception: `get`, `update`,
  and `--issue` flags accept an identifier like `ENG-123` as well.
- **Updates are partial**: `update` verbs only change the flags you pass;
  omitted flags keep their value. Fields cannot be cleared to null.
<!-- /foac-provider:linear -->
<!-- foac-provider:github -->
- **GitHub pagination**: `list` verbs take `--limit N` (default 50, maximum
  100) and `--page N`; output is `{"items":[...],"pageInfo":{...}}`. Follow
  `nextPage` while `hasNextPage` is true.
- **GitHub repositories**: repository-scoped commands take `--repo OWNER/NAME`
  anywhere after the resource name (`issue`, `pull`, ...); omit it inside a git
  checkout whose `origin` (or another remote) points to github.com. `repo list`
  is account-wide and takes no `--repo`.
- **GitHub identifiers**: commands use GitHub numbers, database IDs, usernames,
  refs, names, or file names as described by their help. `release get` requires
  an explicit `--id` or `--tag`, so numeric tags remain unambiguous.
- **Long Markdown**: GitHub commands accept mutually exclusive `--body` and
  `--body-file`. Nested API structures use native JSON flags such as
  `--comments-json`, `--inputs-json`, and `--rules-json`. In `--rules-json`,
  `required_status_checks.contexts` is deprecated; use
  `required_status_checks.checks` with `{"context": ..., "app_id": ...}`.
- **Metadata only**: GitHub release assets, Actions artifacts, and run jobs are
  JSON metadata. Binary uploads/downloads and log streaming are not supported.
<!-- /foac-provider:github -->
<!-- foac-provider:jira -->
- **Jira pagination**: `issue list` takes `--limit N` and `--after TOKEN`;
  follow `pageInfo.nextPageToken` while `hasNextPage` is true. Other list
  verbs take `--limit N` and `--start-at N`; follow `pageInfo.nextStartAt`.
  Output is `{"items":[...],"pageInfo":{...}}`.
- **Jira identifiers**: issues use keys like `ENG-123`. Projects accept a key or
  numeric ID; issue types and priorities accept a name or numeric ID; assignees
  are account IDs (find them with `user list --query`); `--board` accepts a
  numeric ID or an exact board name. `issue list --jql` takes raw JQL; Jira
  rejects unbounded queries, so without `--jql` it defaults to
  `created >= -30d ORDER BY created DESC`. To change status, list options with
  `transition list --issue ENG-123`, then
  `issue transition ENG-123 --to <transition id, transition name, or destination status name>`.
- **Jira text**: issue descriptions and comments accept mutually exclusive
  `--body` and `--body-file` (plain text or Jira wiki markup).
<!-- /foac-provider:jira -->
<!-- foac-provider:confluence -->
- **Confluence pagination**: `space`, `page`, and `comment` lists take
  `--limit N` and an opaque `--after CURSOR`; follow `pageInfo.endCursor`
  while `hasNextPage` is true. `search` is offset-paged with `--limit N` and
  `--start-at N`; follow `pageInfo.nextStartAt`. Output is
  `{"items":[...],"pageInfo":{...}}`.
- **Confluence identifiers**: spaces accept a key like `ENG` or a numeric ID;
  pages and comments use numeric IDs (find pages with `page list --space` or
  `search --cql`). `search --cql` takes raw CQL such as
  `type = page AND text ~ "login"`.
- **Confluence text**: page and comment bodies are written as Confluence wiki
  markup via mutually exclusive `--body` and `--body-file`, and read back in
  the storage representation. `page update` and `comment update` fetch the
  current version internally and re-send omitted fields unchanged, so there is
  no version flag to manage.
<!-- /foac-provider:confluence -->
<!-- foac-provider:firecrawl -->
- **Firecrawl output**: `scrape` prints Firecrawl's raw
  `{"success": true, "data": {...}}` with one key per requested format
  (`data.markdown`, `data.links`, `data.json`, ...) plus `data.metadata`.
  `map`, `search`, and `crawl list` are single-page foac lists
  (`{"items":[...],"pageInfo":{"hasNextPage":false}}`); `search` flattens the
  requested sources into one list, web first. Every other response is raw
  Firecrawl JSON. Credits are spent per page scraped; check
  `team credit-usage` before large crawls.
- **Firecrawl scraping**: `scrape URL` returns markdown by default; pass
  `--formats markdown,html,rawHtml,links,images,screenshot,summary,branding`
  (comma-separated) for more, `--json-prompt`/`--json-schema[-file]` for
  structured extraction (the `json` format, the replacement for the retired
  extract endpoint), and `--only-main-content false` for the whole page.
  Pipe a `map` or `search` list into `scrape --from url` to scrape every
  result. The same per-page flags apply to `crawl create` and
  `batch-scrape create`.
- **Firecrawl jobs**: `crawl`, `batch-scrape`, and `agent` are asynchronous.
  `create` returns `{"id": ...}` at once; `get ID` reads the job's status
  and result (scraped pages in `data` for crawl and batch-scrape, the
  agent's answer in `data` for agent); `cancel ID` stops it. Crawl and
  batch-scrape also have `errors ID` (failed pages) and `get --skip N`: a
  status with a `next` URL has more pages, pass its `skip` value. Add
  `--wait [--poll-interval S] [--wait-timeout S]` to `create` to block until
  the job is `completed`, `failed`, or `cancelled` and print that final
  status with `id` added; a timed-out or failed wait reports the job ID on
  stderr so the job can still be fetched or cancelled.
- **Firecrawl agents**: `agent create "PROMPT" [--url URL]... [--schema JSON]
  [--max-credits N]` runs a browsing agent; results land in `data` once
  `completed`. Agents spend tokens (`team token-usage`) on top of credits.
<!-- /foac-provider:firecrawl -->
<!-- foac-provider:fireflies -->
- **Fireflies transcripts**: `transcript list` returns meeting metadata only;
  `transcript get ID` adds attendees, channels, and the AI `summary`
  (overview, `action_items`, keywords, outline). Add `--sentences` to include
  the spoken transcript, which is large. Audio and video URLs and meeting
  analytics are not fetched: they need a paid plan, and one gated field
  fails the whole request on a Free plan.
- **Fireflies pagination**: `transcript list` and `bite list` take `--limit N`
  (at most 50, the default) and `--start-at N`; output is
  `{"items":[...],"pageInfo":{"hasNextPage":...,"nextStartAt":...}}`. Follow
  `pageInfo.nextStartAt` while `hasNextPage` is true. Other lists return
  everything in one page. Fireflies rate-limits hard (50 requests a day on
  Free, 500 on Pro), so filter lists instead of paging through everything.
- **Fireflies search**: `transcript list --keyword WORD` searches titles;
  `--scope sentences|all` searches what was said. Filter with
  `--from-date`/`--to-date` (ISO 8601), repeatable `--organizer` and
  `--participant` emails, `--channel ID`, `--user ID`, or `--mine`.
- **Fireflies AskFred**: `askfred create "QUESTION"` answers from your
  meetings. Pin it to one meeting with `--transcript ID`, or filter with
  `--start-time`/`--end-time` (ISO 8601; the window defaults to the 30 days
  before the end time) and repeatable `--organizer`, `--participant`, and
  `--channel`. The answer is in `message.answer`, and
  `message.thread_id` feeds `askfred continue THREAD_ID "FOLLOW-UP"`.
- **Fireflies identifiers**: transcripts, threads, bites, and channels use
  opaque string IDs from their lists; `user get me` is the API key's owner.
<!-- /foac-provider:fireflies -->
<!-- foac-provider:neon -->
- **Neon project**: pass `--project ID` anywhere after `neon`, or set
  `NEON_PROJECT_ID`; only `org list` and `project list` work without it. Neon
  requires an organization ID on `project list` when the account belongs to
  an organization: pass `--org ID` or set `NEON_ORG_ID`, finding IDs with
  `org list`.
- **Neon pagination**: `project list`, `branch list`, and `operation list`
  take `--limit N` (default 50) and an opaque `--after CURSOR`; output is
  `{"items":[...],"pageInfo":{...}}`. Follow `pageInfo.endCursor` while
  `hasNextPage` is true. Other lists are not paginated.
- **Neon identifiers**: branches use IDs like `br-...` and compute endpoints
  IDs like `ep-...`; find them with `branch list` and `endpoint list`.
  `connection-uri` requires `--database` and `--role` and prints a URI
  containing that role's password.
- **Neon operations are asynchronous**: `branch create|delete` and
  `endpoint start|suspend|restart` return `operations` still running; poll
  `operation get ID` until `status` is `finished` before using the result.
  `branch delete` refuses the default branch and branches with children.
<!-- /foac-provider:neon -->
<!-- foac-provider:notion -->
- **Notion content**: page bodies are Notion's enhanced Markdown.
  `page get ID --markdown` returns `{"object": "page_markdown", "markdown":
  ...}`; without it, `page get` returns the page object (properties only).
  `page create` and `page update` take `--body`/`--body-file`; on update the
  body replaces all page content (Notion refuses if that would delete child
  pages or databases). On create, a leading `# Heading` becomes the page
  title and is dropped from the content. Blocks stay Notion-native JSON:
  `block append ID --children '[{"paragraph": {"rich_text": [{"text":
  {"content": "Hi"}}]}}]'` (or `--children-file`), with `--after BLOCK_ID`
  to insert mid-page.
- **Notion databases**: a database is a container of one or more data
  sources; rows live in data sources. `database get ID` lists
  `data_sources[].id`; `data-source query ID` reads rows, with Notion's raw
  `--filter` object and `--sorts` array as JSON. `data-source get ID` shows
  the property schema. Create a row with
  `page create --data-source ID --title ... --properties JSON`, keys being
  property names or IDs.
- **Notion properties**: `--title` sets the title property whatever the
  column is named; `--properties` takes Notion's raw property-value object,
  e.g. `{"Status": {"status": {"name": "Done"}}}`.
- **Notion pagination**: every list takes `--limit N` (default 50, at most
  100) and an opaque `--after CURSOR`; output is
  `{"items":[...],"pageInfo":{"hasNextPage":...,"endCursor":...}}`. Follow
  `pageInfo.endCursor` while `hasNextPage` is true. `block list` returns one
  level; list a block with `has_children: true` to go deeper.
- **Notion identifiers**: IDs are UUIDs, with or without dashes; the 32-hex
  suffix of a Notion URL is the page or database ID. `page list`,
  `data-source list`, and `search` are title searches over what the token
  can see. Comments: `comment list PAGE_OR_BLOCK_ID` shows open comments;
  reply with `comment create --discussion DISCUSSION_ID`.
<!-- /foac-provider:notion -->
<!-- foac-provider:sentry -->
- **Sentry organization**: pass `--org SLUG` anywhere after `sentry`, or set
  `SENTRY_ORG`; only `org list` works without it. `--org` and `--project`
  take a slug or a numeric ID; IDs survive renames. On a TTY,
  `foac auth sentry login` first asks for the Sentry hostname (default
  `sentry.io`, always https) and saves it; with redirected stdin it reads only
  the token, so pass `--host HOSTNAME` to save a self-hosted host
  non-interactively. The host is saved with the instance's credentials;
  `SENTRY_URL` overrides it for the default instance only and is also
  normalized to https.
- **Sentry pagination**: `list` verbs take `--cursor CURSOR`; output is
  `{"items":[...],"pageInfo":{...}}`. Follow `nextCursor` while `hasNextPage`
  is true.
- **Sentry issues**: `issue` and `--issue` accept a numeric issue ID or a
  short ID like `PROJ-123`. `issue list` searches the organization, or one
  project with `--project SLUG`; `--query` takes Sentry search syntax such as
  `is:unresolved release:1.2.0`. Releases are read-only; use `sentry-cli` to
  create releases and upload sourcemaps.
<!-- /foac-provider:sentry -->
<!-- foac-provider:slack -->
- **Slack pagination**: list and search commands take `--limit N` (default 100)
  and `--after CURSOR`; output is
  `{"items":[...],"pageInfo":{"hasNextPage":...,"endCursor":...}}`.
  Follow `endCursor` while `hasNextPage` is true.
- **Slack names**: conversation arguments accept an ID or a channel name such
  as `#eng`; user get accepts an ID, `@name`, display name, or email. Name
  resolution pages through the visible workspace directory. Email lookup
  requires `users:read.email`.
- **Slack messages**: `message list/get/create` accept `--thread-ts`; list reads
  replies and create posts a reply. Message text uses mutually exclusive
  `--body` and `--body-file`. Update and delete work only on messages posted by
  the selected identity (the bot when available, otherwise the user). Pass
  reaction names with or without surrounding colons.
<!-- /foac-provider:slack -->
<!-- foac-provider:vercel -->
- **Vercel scope**: omit `--team` for the token's personal account, or pass a
  team ID like `team_...` anywhere after `vercel`. `VERCEL_TEAM_ID` is the
  default. Find team IDs with `team list`.
- **Vercel pagination**: list verbs take `--limit N` (default 20) and
  `--after CURSOR`; output is `{"items":[...],"pageInfo":{...}}`. Follow
  `pageInfo.endCursor` while `hasNextPage` is true. Vercel cursors are usually
  millisecond timestamps, but pass them back unchanged.
- **Vercel identifiers**: projects accept an ID or name; deployments accept an
  ID (and `get` also accepts a URL); domains use their DNS name. Project
  updates change only supplied fields. Deployment creation/uploads, logs, DNS
  records, and environment variables are not covered.
<!-- /foac-provider:vercel -->

<!-- foac-provider:airtable -->
## Airtable resources

- Discovery: `base list`, `table list --base APP_ID` (the schema).
- Data: `record list|get|create|update|delete`.
- Discussion: `comment list|create` (`--parent COMMENT_ID` replies).

Use `foac airtable <resource> --help` for flags and required arguments. Base,
table, and field creation or changes, batch and upsert writes, attachment
uploads, comment edits and deletes, and webhooks are not covered.
<!-- /foac-provider:airtable -->

<!-- foac-provider:axiom -->
## Axiom resources

- Data: `dataset list|get|create|update|delete|trim`,
  `field list|get` (both take `--dataset`), `query`, `ingest`.
- Annotations: `annotation list|get|create|update|delete`.
- Alerting (read-only): `monitor list|get|history`, `notifier list|get`.
- Directory: `user list|get`, `org list|get`.

Use `foac axiom <resource> --help` for flags and required arguments. API
tokens, dashboards, virtual fields, saved queries, views, roles, live
streaming, and CSV ingest are not covered.
<!-- /foac-provider:axiom -->

<!-- foac-provider:github -->
## GitHub resources

- Collaboration: `repo`, `issue`, `comment`, `pull`, `review`.
- Actions: `workflow`, `run`.
- Git and checks: `branch`, `ref`, `branch-protection`, `commit`,
  `commit-comment`, `status`, `check-run`, `check-suite`.
- Administration: `release`, `release-asset`, `artifact`, `label`,
  `collaborator`.

Use `foac github <resource> --help` for the available verbs and flags. GitHub
issue lists exclude pull requests even though the upstream issues endpoint
returns both. Review creation accepts inline comments as a GitHub-native JSON
array through `--comments-json`; omit `--event` to create a pending review,
then use `review submit`.
<!-- /foac-provider:github -->

<!-- foac-provider:jira -->
## Jira resources

- Issues: `issue list|get|create|update|transition` and
  `comment list|create|update|delete`.
- Structure: `project list|get` and `sprint list|get` (sprints need
  `--board`).
- Directory and workflow: `user list|get` and `transition list`.

Use `foac jira <resource> --help` for flags and required arguments. Issue
`update` changes only the supplied fields; `--label` replaces the full label
list.
<!-- /foac-provider:jira -->

<!-- foac-provider:confluence -->
## Confluence resources

- Spaces: `space list|get`.
- Pages: `page list|get|create|update|delete` (`create` takes `--space`,
  `--title`, and optional `--parent`).
- Footer comments: `comment list|create|update|delete` (`list` and `create`
  take `--page`).
- Discovery: `search --cql`.

Use `foac confluence <resource> --help` for flags and required arguments.
Inline comments, attachments, whiteboards, and databases are not covered.
<!-- /foac-provider:confluence -->

<!-- foac-provider:firecrawl -->
## Firecrawl resources

- Synchronous: `scrape URL`, `map URL`, `search QUERY`.
- Jobs: `crawl list|create|get|errors|cancel`,
  `batch-scrape create|get|errors|cancel`, `agent create|get|cancel`.
- Usage: `team credit-usage|token-usage|queue-status|concurrency`.

Use `foac firecrawl <resource> --help` for flags. Local file parsing,
monitors, browser sessions, and page interaction are not covered.
<!-- /foac-provider:firecrawl -->

<!-- foac-provider:fireflies -->
## Fireflies resources

- Meetings: `transcript list|get|create|update|delete` (`create --url`
  transcribes a public media URL on a paid plan; `update --title` renames).
- Questions: `askfred list|get|create|continue|delete`.
- Clips: `bite list|get`.
- Directory: `channel list|get`, `contact list`, `user list|get`.

Use `foac fireflies <resource> --help` for flags and required arguments.
Live meetings (add the bot, pause recording, live action items), sharing and
privacy changes, AI App outputs, analytics, audit events, and webhooks are
not covered.
<!-- /foac-provider:fireflies -->

<!-- foac-provider:notion -->
## Notion resources

- Pages: `page list|get|create|update|delete` (`delete` moves to the trash;
  `create` takes `--parent PAGE_ID` or `--data-source ID`, or neither for a
  private workspace page with a personal access token).
- Databases: `database get`; rows: `data-source list|get|query`.
- Content: `block list|get|append`.
- Discussion: `comment list|create`.
- Discovery and directory: `search`, `user list|get` (`user get me` is the
  token's bot user; personal access tokens cannot list users).

Use `foac notion <resource> --help` for flags and required arguments. File
uploads, views, data source and database schema changes, block updates and
deletes, and webhooks are not covered.
<!-- /foac-provider:notion -->

<!-- foac-provider:slack -->
## Slack resources

- Conversations: `conversation list|get`.
- Messages: `message list|get|create|update|delete`, including threads.
- Directory and discovery: `user list|get` and `search`.
- Reactions: `reaction add|remove`.

Use `foac slack <resource> --help` for flags and required arguments. Typical bot
scopes are `channels:history`, `channels:read`, `chat:write`, `groups:history`,
`groups:read`, `im:history`, `im:read`, `mpim:history`, `mpim:read`,
`reactions:write`, `users:read`, and `users:read.email`; only grant scopes for
the commands the app needs. User-only operation needs equivalent user scopes;
search uses the user credential (environment or config), never the bot token.
<!-- /foac-provider:slack -->

<!-- foac-provider:vercel -->
## Vercel resources

- Scope discovery: `team list|get`.
- Projects: `project list|get|create|update|delete`.
- Deployments: `deployment list|get|cancel|delete`.
- Account domains: `domain list|get|config|create|delete`.
- Project domains: `project-domain list|get|create|update|delete|verify`.

Use `foac vercel <resource> --help` for flags and required arguments. Domain
ownership and assignment are separate: `domain` manages the account-level
domain, while `project-domain` assigns a domain to a project.
<!-- /foac-provider:vercel -->

## Examples

```sh
foac auth status
# cross-provider join (works for any list | any get pair):
foac linear user list | foac slack user get --from email
foac github repo list | foac vercel project get --from name
```

<!-- foac-provider:airtable -->

```sh
foac airtable base list
foac airtable table list --base appAbc123
foac airtable record list --base appAbc123 --table Tasks --view "Open" --formula "{Owner} = 'Lolo'" --field Name --field Due
foac airtable record create --base appAbc123 --table Tasks --fields '{"Name": "Ship it", "Status": "Todo"}' --typecast
foac airtable record update --base appAbc123 --table Tasks recXyz789 --fields '{"Status": "Done"}'
foac airtable comment create --base appAbc123 --table Tasks recXyz789 --body "Done, see PR #42"
```

<!-- /foac-provider:airtable -->

<!-- foac-provider:axiom -->

```sh
foac axiom dataset list
foac axiom field list --dataset logs
foac axiom query "['logs'] | where level == 'error' | where _time > ago(1h) | limit 20"
foac axiom ingest logs --events-file events.ndjson --timestamp-field ts
foac axiom annotation create --type deploy --dataset logs --title "v1.2.0" --url https://github.com/owner/repo/pull/42
```

<!-- /foac-provider:axiom -->

<!-- foac-provider:linear -->

```sh
foac linear issue list --team ENG --state "In Progress"
foac linear issue create --team <TEAM_UUID> --title "Fix login" --description "..."
foac linear comment create --issue ENG-123 --body "Done, see PR #42"
# ENG-1 blocks ENG-2 (ENG-2 depends on ENG-1); `issue get` lists them under
# relations (outgoing) and inverseRelations (incoming, e.g. blocked by)
foac linear relation create --issue ENG-1 --type blocks --related ENG-2
```

<!-- /foac-provider:linear -->

<!-- foac-provider:github -->

```sh
foac github issue list --repo owner/repo --state open
foac github pull create --repo owner/repo --head feature --base main --title "Add feature" --body-file /tmp/pr.md
foac github review create --repo owner/repo --pull 42 --event approve --body "Looks good"
foac github run rerun --repo owner/repo 123456 --failed
```

<!-- /foac-provider:github -->

<!-- foac-provider:jira -->

```sh
foac jira issue list --jql 'project = ENG AND statusCategory != Done'
foac jira issue create --project ENG --type Task --summary "Fix login" --body "Steps..."
foac jira issue transition ENG-123 --to "In Progress"
foac jira comment create --issue ENG-123 --body "Done, see PR #42"
```

<!-- /foac-provider:jira -->

<!-- foac-provider:confluence -->

```sh
foac confluence page list --space ENG
foac confluence page create --space ENG --title "Runbook" --body-file /tmp/runbook.wiki
foac confluence comment create --page 12345 --body "Updated, see PR #42"
foac confluence search --cql 'type = page AND text ~ "login"'
```

<!-- /foac-provider:confluence -->

<!-- foac-provider:firecrawl -->

```sh
foac firecrawl scrape https://docs.example.com/api --formats markdown,links
foac firecrawl map https://docs.example.com --search "authentication" | foac firecrawl scrape --from url
foac firecrawl search "rust async runtime comparison" --limit 5 --tbs qdr:y
foac firecrawl crawl create https://docs.example.com --limit 50 --include-paths "/docs/*" --wait
foac firecrawl agent create "List the pricing tiers and their monthly prices" --url https://example.com/pricing --wait
```

<!-- /foac-provider:firecrawl -->

<!-- foac-provider:fireflies -->

```sh
foac fireflies transcript list --from-date 2026-09-01T00:00:00.000Z --participant alex@example.com
foac fireflies transcript get 01K2EXAMPLE
foac fireflies transcript list --keyword pricing --scope sentences --limit 5 | foac fireflies transcript get --from id
foac fireflies askfred create "What did we decide about the Q4 launch?" --start-time 2026-09-01T00:00:00Z
foac fireflies askfred continue THREAD_ID "Who owns the follow-ups?"
```

<!-- /foac-provider:fireflies -->

<!-- foac-provider:neon -->

```sh
foac neon branch list --project proj-1
foac neon branch create --project proj-1 --name preview --parent br-main-123
foac neon endpoint suspend ep-123 --project proj-1
foac neon connection-uri --project proj-1 --database app --role app_owner
```

<!-- /foac-provider:neon -->

<!-- foac-provider:notion -->

```sh
foac notion search "launch spec"
foac notion page get 1a2b3c4d5e6f47a8b9c0d1e2f3a4b5c6 --markdown
foac notion page create --parent 1a2b3c4d5e6f47a8b9c0d1e2f3a4b5c6 --title "Runbook" --body-file /tmp/runbook.md
foac notion database get 2b3c4d5e6f7a48b9c0d1e2f3a4b5c6d7
foac notion data-source query 3c4d5e6f7a8b49c0d1e2f3a4b5c6d7e8 --filter '{"property": "Status", "status": {"equals": "In progress"}}'
foac notion comment create --page 1a2b3c4d5e6f47a8b9c0d1e2f3a4b5c6 --body "Updated, see PR #42"
```

<!-- /foac-provider:notion -->

<!-- foac-provider:sentry -->

```sh
foac sentry issue list --org acme --project backend --query "is:unresolved"
foac sentry issue latest-event PROJ-123 --org acme
foac sentry issue update PROJ-123 --org acme --status resolved
```

<!-- /foac-provider:sentry -->

<!-- foac-provider:slack -->

```sh
foac slack conversation get '#eng'
foac slack message list '#eng' --limit 50
foac slack message create '#eng' --body "PR is up: https://github.com/owner/repo/pull/42"
foac slack message create '#eng' --thread-ts 1724432400.123456 --body-file /tmp/reply.md
foac slack search 'deployment in:eng' --sort timestamp --direction desc
foac slack reaction add '#eng' 1724432400.123456 eyes
```

<!-- /foac-provider:slack -->

<!-- foac-provider:vercel -->

```sh
foac vercel team list
foac vercel project list --team team_123 --search web
foac vercel deployment list --project web --state READY
foac vercel project-domain create --project web preview.example.com --git-branch preview
```

<!-- /foac-provider:vercel -->

## Maintenance

`foac skill install` and `foac update` report byte-identical skills as
`Unchanged` without rewriting them. `foac update` replaces the binary with the
latest release and refreshes any foac provider skills already installed in
`~/.claude/skills` or `~/.agents/skills` — on a Homebrew install it exits 1 and
points at `brew upgrade foac`. However the binary was upgraded, the new version
refreshes those installed skills on its first run, so they always match the
running CLI. `foac version` (or `foac --version`) prints `foac <version>`; `foac about` prints the brand banner, version, and repository URL.
foac checks
GitHub at most once a day and prints a two-line notice on stderr while a newer
release exists. It never auto-installs, and the notice is not JSON. Set
`FOAC_NO_UPDATE_CHECK` (or `CI`) to disable it.
